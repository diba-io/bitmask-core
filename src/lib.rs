#[macro_use]
extern crate amplify;

#[cfg(not(target_arch = "wasm32"))]
extern crate amplify_legacy;

use std::str::FromStr;

use amplify::hex::ToHex;
use anyhow::Result;
use bdk::{wallet::AddressIndex, FeeRate, LocalUtxo};

use bitcoin::EcdsaSighashType;
use bitcoin_blockchain::locks::LockTime;
use bitcoin_blockchain::locks::SeqNo;
use bitcoin_scripts::PubkeyScript;
use bp::Txid;
use data::constants::BITCOIN_ELECTRUM_API;
use data::structs::AddressAmount;
use data::structs::InvoiceResult;
use data::structs::IssueResult;
use data::structs::PsbtRequest;
use data::structs::PsbtResult;
use data::structs::RgbTransferRequest;
use data::structs::RgbTransferResult;
use data::structs::{AcceptRequest, AcceptResponse};
use miniscript_crate::Descriptor;
use operations::rgb::constants::RGB_PSBT_TAPRET;
use operations::rgb::pay::pay_asset as pay_rgb_asset;
use operations::rgb::resolvers::ExplorerResolver;
use operations::rgb::{
    invoice::create_invoice as create_rgb_invoice, issue::issue_contract as create_contract,
    psbt::create_psbt as create_rgb_psbt, schemas::default_fungible_iimpl,
};
use psbt::ProprietaryKeyDescriptor;
use psbt::ProprietaryKeyLocation;
use psbt::ProprietaryKeyType;

use psbt::Psbt;
use rgbstd::containers::BindleContent;
use rgbstd::contract::ContractId;
use rgbstd::contract::GraphSeal;

use bitcoin::{util::address::Address, OutPoint, Transaction}; // Shared
use bitcoin_hashes::{sha256, Hash};
use rgbstd::persistence::Inventory;
use rgbstd::persistence::Stash;
use rgbstd::persistence::Stock;
use rgbwallet::RgbInvoice;
use serde::Deserialize;
use serde::Serialize;
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use strict_encoding::StrictSerialize;
use strict_encoding::TypeName;
use tokio::try_join;
use wallet::descriptors::InputDescriptor;
use wallet::hd::DerivationAccount;
use wallet::hd::UnhardenedIndex;

pub mod data;
pub mod operations;
pub mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

// Shared
pub use crate::{
    data::{
        constants::{get_endpoint, get_network, switch_host, switch_network},
        structs::{
            BlindingUtxo, EncryptedWalletData, FullUtxo, FundVaultDetails, SatsInvoice, ThinAsset,
            TransferResult, TransfersRequest, TransfersSerializeResponse, WalletData,
            WalletTransaction,
        },
    },
    operations::{
        bitcoin::{
            create_payjoin, create_transaction, dust_tx, get_wallet, new_mnemonic, save_mnemonic,
            sign_psbt, synchronize_wallet,
        },
        lightning,
    },
};

impl SerdeEncryptSharedKey for EncryptedWalletData {
    type S = BincodeSerializer<Self>; // you can specify serializer implementation (or implement it by yourself).
}

// Wallet Operations
pub fn get_encrypted_wallet(
    password: &str,
    encrypted_descriptors: &str,
) -> Result<EncryptedWalletData> {
    // read hash digest and consume hasher
    let hash = sha256::Hash::hash(password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();
    let encrypted_descriptors: Vec<u8> = hex::decode(encrypted_descriptors)?;
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors)?;
    Ok(EncryptedWalletData::decrypt_owned(
        &encrypted_message,
        &SharedKey::from_array(shared_key),
    )?)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MnemonicSeedData {
    pub mnemonic: String,
    pub serialized_encrypted_message: String,
}

// TODO: should probably be called "new_mnemonic_seed"
pub fn get_mnemonic_seed(
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let encrypted_wallet_data = new_mnemonic(seed_password)?;
    let encrypted_message = encrypted_wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let serialized_encrypted_message = hex::encode(encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: encrypted_wallet_data.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub fn save_mnemonic_seed(
    mnemonic_phrase: &str,
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let vault_data = save_mnemonic(mnemonic_phrase, seed_password)?;
    let encrypted_message = vault_data.encrypt(&SharedKey::from_array(shared_key))?;
    let serialized_encrypted_message = hex::encode(encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: vault_data.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub async fn get_wallet_data(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<WalletData> {
    info!("get_wallet_data");
    info!("descriptor:", &descriptor);
    info!("change_descriptor:", format!("{:?}", &change_descriptor));

    let wallet = get_wallet(descriptor, change_descriptor)?;
    synchronize_wallet(&wallet).await?;
    let address = wallet.get_address(AddressIndex::LastUnused)?.to_string();
    info!(format!("address: {address}"));
    let balance = wallet.get_balance()?;
    info!(format!("balance: {balance:?}"));
    let utxos = wallet.list_unspent().unwrap_or_default();
    let utxos: Vec<String> = utxos.into_iter().map(|x| x.outpoint.to_string()).collect();
    trace!(format!("unspent: {utxos:#?}"));

    let mut transactions = wallet.list_transactions(false).unwrap_or_default();
    trace!(format!("transactions: {transactions:#?}"));

    transactions.sort_by(|a, b| {
        b.confirmation_time
            .as_ref()
            .map(|t| t.height)
            .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
    });

    let transactions: Vec<WalletTransaction> = transactions
        .into_iter()
        .map(|tx| WalletTransaction {
            txid: tx.txid,
            received: tx.received,
            sent: tx.sent,
            fee: tx.fee,
            confirmed: tx.confirmation_time.is_some(),
            confirmation_time: tx.confirmation_time,
        })
        .collect();

    Ok(WalletData {
        address,
        balance,
        transactions,
        utxos,
    })
}

pub async fn get_new_address(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<String> {
    info!("get_new_address");
    info!(format!("descriptor: {descriptor}"));
    info!(format!("change_descriptor: {change_descriptor:?}"));

    let wallet = get_wallet(descriptor, change_descriptor)?;
    synchronize_wallet(&wallet).await?;
    let address = wallet.get_address(AddressIndex::New)?.to_string();
    info!(format!("address: {address}"));
    Ok(address)
}

pub async fn send_sats(
    descriptor: &str,
    change_descriptor: &str,
    destination: &str, // bip21 uri or address
    amount: u64,
    fee_rate: Option<f32>,
) -> Result<Transaction> {
    use payjoin::UriExt;

    let wallet = get_wallet(descriptor, Some(change_descriptor.to_owned()))?;
    synchronize_wallet(&wallet).await?;

    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let transaction = match payjoin::Uri::try_from(destination) {
        Ok(uri) => {
            let address = uri.address.clone();
            if let Ok(pj_uri) = uri.check_pj_supported() {
                create_payjoin(
                    vec![SatsInvoice { address, amount }],
                    &wallet,
                    fee_rate,
                    pj_uri,
                )
                .await?
            } else {
                create_transaction(vec![SatsInvoice { address, amount }], &wallet, fee_rate).await?
            }
        }
        _ => {
            let address = Address::from_str(destination)?;
            create_transaction(vec![SatsInvoice { address, amount }], &wallet, fee_rate).await?
        }
    };

    Ok(transaction)
}

pub async fn fund_vault(
    btc_descriptor_xprv: &str,
    btc_change_descriptor_xprv: &str,
    assets_address: &str,
    uda_address: &str,
    asset_amount: u64,
    uda_amount: u64,
    fee_rate: Option<f32>,
) -> Result<FundVaultDetails> {
    let assets_address = Address::from_str(assets_address)?;
    let uda_address = Address::from_str(uda_address)?;

    let wallet = get_wallet(
        btc_descriptor_xprv,
        Some(btc_change_descriptor_xprv.to_owned()),
    )?;
    synchronize_wallet(&wallet).await?;

    let asset_invoice = SatsInvoice {
        address: assets_address,
        amount: asset_amount,
    };
    let uda_invoice = SatsInvoice {
        address: uda_address,
        amount: uda_amount,
    };

    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let asset_tx_details = create_transaction(
        vec![asset_invoice.clone(), asset_invoice],
        &wallet,
        fee_rate,
    )
    .await?;

    let uda_tx_details =
        create_transaction(vec![uda_invoice.clone(), uda_invoice], &wallet, fee_rate).await?;

    let asset_txid = asset_tx_details.txid();
    let asset_outputs: Vec<String> = asset_tx_details
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{asset_txid}:{i}"))
        .collect();

    let uda_txid = uda_tx_details.txid();
    let uda_outputs: Vec<String> = uda_tx_details
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{uda_txid}:{i}"))
        .collect();

    Ok(FundVaultDetails {
        assets_output: Some(asset_outputs[0].to_owned()),
        assets_change_output: Some(asset_outputs[1].to_owned()),
        udas_output: Some(uda_outputs[0].to_owned()),
        udas_change_output: Some(uda_outputs[1].to_owned()),
    })
}

fn utxo_string(utxo: &LocalUtxo) -> String {
    utxo.outpoint.to_string()
}

pub async fn get_assets_vault(
    rgb_assets_descriptor_xpub: &str,
    rgb_udas_descriptor_xpub: &str,
) -> Result<FundVaultDetails> {
    let assets_wallet = get_wallet(rgb_assets_descriptor_xpub, None)?;
    let udas_wallet = get_wallet(rgb_udas_descriptor_xpub, None)?;

    try_join!(
        synchronize_wallet(&assets_wallet),
        synchronize_wallet(&udas_wallet)
    )?;

    let assets_utxos = assets_wallet.list_unspent()?;
    let uda_utxos = udas_wallet.list_unspent()?;

    debug!(format!("Asset UTXOs: {assets_utxos:#?}"));
    debug!(format!("UDA UTXOs: {uda_utxos:#?}"));

    let mut assets_utxos: Vec<String> = assets_utxos.iter().map(utxo_string).collect();
    assets_utxos.sort();

    let mut uda_utxos: Vec<String> = uda_utxos.iter().map(utxo_string).collect();
    uda_utxos.sort();

    let assets_change_output = assets_utxos.pop();
    let assets_output = assets_utxos.pop();
    let udas_change_output = uda_utxos.pop();
    let udas_output = uda_utxos.pop();

    Ok(FundVaultDetails {
        assets_output,
        assets_change_output,
        udas_output,
        udas_change_output,
    })
}

// RGB Operations
pub async fn issue_contract(
    ticker: &str,
    name: &str,
    description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
    iface: &str,
) -> Result<IssueResult> {
    // TODO: Get stock from Carbonado
    let stock = Stock::default();

    let iface_name = TypeName::from_str(iface).expect("invalid iface name format");
    let iface = stock
        .iface_by_name(&iface_name)
        .expect("invalid iface format");

    // TODO: Provide a way to get iimpl by iface
    let iimpl = default_fungible_iimpl();

    let contract = create_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        seal,
        iface.to_owned(),
        iimpl,
    )?;

    // TODO: Update Stock to Carbonado
    // stock.import_contract(contract, resolver);

    let id = contract.contract_id().to_string();
    let schema_id = contract.schema_id().to_string();
    let genesis = contract.bindle().to_string();

    Ok(IssueResult {
        genesis,
        id: id.clone(),
        asset_id: id,
        schema_id,
    })
}

pub async fn create_invoice(
    contract_id: &str,
    iface: &str,
    amount: u64,
    seal: &str,
) -> Result<InvoiceResult> {
    // TODO: Get stock from Carbonado
    let mut stock = Stock::default();

    let iface_name = TypeName::from_str(iface)?;
    let iface = stock.iface_by_name(&iface_name)?;

    let seal_parts: Vec<&str> = seal.split(':').collect();
    let txid = Txid::from_str(seal_parts[0]).expect("invalid txid");
    let seal = GraphSeal::tapret_first(txid, 0);
    let contract_id = ContractId::from_str(contract_id)?;

    let contract = create_rgb_invoice(contract_id, iface.to_owned(), amount, seal, stock.clone())?;
    let result = contract.to_string();

    // TODO: Update Stock to Carbonado
    // Store Seal into Stock
    stock.store_seal_secret(seal).expect("stock internal error");

    Ok(InvoiceResult { invoice: result })
}

pub async fn create_psbt(request: PsbtRequest) -> Result<PsbtResult> {
    let PsbtRequest {
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
    } = request;

    // TODO: Pull from Carbonado (?)
    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };

    let outpoint: OutPoint = asset_utxo.parse()?;
    let inputs = vec![InputDescriptor {
        outpoint,
        terminal: asset_utxo_terminal.parse()?,
        seq_no: SeqNo::default(),
        tweak: None,
        sighash_type: EcdsaSighashType::All,
    }];

    let bitcoin_addresses: Vec<AddressAmount> = bitcoin_changes
        .into_iter()
        .map(|btc| AddressAmount::from_str(btc.as_str()).expect("invalid AddressFormat parse"))
        .collect();

    let outputs: Vec<(PubkeyScript, u64)> = bitcoin_addresses
        .into_iter()
        .map(|AddressAmount { address, amount }| (address.script_pubkey().into(), amount))
        .collect();

    let descriptor: &Descriptor<DerivationAccount> = &Descriptor::from_str(&descriptor_pub)?;
    let props = vec![ProprietaryKeyDescriptor {
        location: ProprietaryKeyLocation::Output(outpoint.vout as u16),
        ty: ProprietaryKeyType {
            prefix: RGB_PSBT_TAPRET.to_owned(),
            subtype: outpoint.vout as u8,
        },
        key: None,
        value: None,
    }];

    let lock_time = LockTime::anytime();
    let change_index = match change_index {
        Some(index) => UnhardenedIndex::from_str(index.as_str())?,
        _ => UnhardenedIndex::default(),
    };

    let psbt_file = create_rgb_psbt(
        descriptor,
        lock_time,
        inputs,
        outputs,
        props,
        change_index,
        fee,
        &tx_resolver,
    )?;

    // TODO: Push to Carbonado (?)
    let psbt = PsbtResult {
        psbt: psbt::serialize::Serialize::serialize(&psbt_file).to_hex(),
    };
    Ok(psbt)
}

pub async fn pay_asset(request: RgbTransferRequest) -> Result<RgbTransferResult> {
    let RgbTransferRequest { rgb_invoice, psbt } = request;

    // TODO: Pull from Carbonado
    let stock = Stock::default();

    let invoice = RgbInvoice::from_str(&rgb_invoice)?;
    let psbt_file = Psbt::from_str(&psbt)?;

    let transfer = pay_rgb_asset(invoice, psbt_file, stock)?;

    // TODO: Push to Carbonado
    let consig = RgbTransferResult {
        consig_id: transfer.bindle_id().to_string(),
        consig: transfer
            .to_strict_serialized::<0xFFFFFF>()
            .expect("invalid transfer serialization")
            .to_hex(),
    };
    Ok(consig)
}

pub async fn accept_transfer(request: AcceptRequest) -> Result<AcceptResponse> {
    let AcceptRequest { consignment } = request;

    // TODO: Pull from Carbonado
    let stock = Stock::default();

    // TODO: Push to Carbonado
    let resp = AcceptResponse {
        contract_id: todo!(),
        valid: todo!(),
        info: todo!(),
    };

    Ok(resp)
}
