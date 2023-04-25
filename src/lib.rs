#[macro_use]
extern crate amplify;
use std::str::FromStr;

use amplify::hex::ToHex;
use anyhow::Result;
pub use bdk::TransactionDetails;
use bdk::{wallet::AddressIndex, FeeRate, LocalUtxo};
use data::constants::BITCOIN_ELECTRUM_API;

// RGB Imports
use data::structs::{
    AcceptRequest, AcceptResponse, ContractDetail, ContractsResponse, InterfaceDetail,
    InterfacesResponse, InvoiceResult, IssueResponse, PsbtRequest, PsbtResponse,
    RgbTransferRequest, RgbTransferResponse, SchemaDetail, SchemasResponse,
};
use operations::rgb::psbt::extract_commit;
use operations::rgb::{
    invoice::{accept_payment, create_invoice as create_rgb_invoice, pay_invoice},
    issue::issue_contract as create_contract,
    psbt::create_psbt as create_rgb_psbt,
    resolvers::ExplorerResolver,
};
use rgbstd::containers::BindleContent;
use rgbstd::persistence::{Inventory, Stash, Stock};

use bitcoin::util::address::Address;
use bitcoin_hashes::{sha256, Hash};

use serde::Deserialize;
use serde::Serialize;
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use strict_encoding::StrictSerialize;
use tokio::try_join;
pub mod data;
pub mod operations;
pub mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

// Shared
pub use crate::{
    data::{
        constants::{get_env, get_network, set_env, switch_network},
        structs::{
            EncryptedWalletData, FundVaultDetails, SatsInvoice, WalletData, WalletTransaction,
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

pub fn new_mnemonic_seed(
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let encrypted_wallet_data = new_mnemonic(seed_password)?;
    let encrypted_message = encrypted_wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let serialized_encrypted_message = hex::encode(encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: encrypted_wallet_data.private.mnemonic,
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
        mnemonic: vault_data.private.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub async fn get_wallet_data(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<WalletData> {
    info!("get_wallet_data");
    info!(format!("descriptor: {descriptor}"));
    info!(format!("change_descriptor {change_descriptor:?}"));

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
) -> Result<TransactionDetails> {
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

    let asset_txid = asset_tx_details.txid;
    let asset_outputs: Vec<String> = asset_tx_details
        .transaction
        .expect("asset tx exists")
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{asset_txid}:{i}"))
        .collect();

    let uda_txid = uda_tx_details.txid;
    let uda_outputs: Vec<String> = uda_tx_details
        .transaction
        .expect("uda tx exists")
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
        is_funded: true,
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

    let is_funded = assets_change_output.is_some()
        && assets_output.is_some()
        && udas_change_output.is_some()
        && udas_output.is_some();

    Ok(FundVaultDetails {
        assets_output,
        assets_change_output,
        udas_output,
        udas_change_output,
        is_funded,
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
) -> Result<IssueResponse> {
    // TODO: Get stock from Carbonado
    let mut stock = Stock::default();

    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };

    let contract = create_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        iface,
        seal,
        tx_resolver,
        &mut stock,
    )?;

    let contract_id = contract.contract_id().to_string();
    let genesis = contract.bindle().to_string();

    // TODO: Update Stock to Carbonado
    Ok(IssueResponse {
        contract_id,
        iface: iface.to_string(),
        genesis,
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
    let invoice = create_rgb_invoice(contract_id, iface, amount, seal, &mut stock)?;

    // TODO: Update Stock to Carbonado
    Ok(InvoiceResult {
        invoice: invoice.to_string(),
    })
}

pub async fn create_psbt(request: PsbtRequest) -> Result<PsbtResponse> {
    let PsbtRequest {
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
        input_tweak,
    } = request;

    // TODO: Pull from Carbonado (?)
    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };

    let psbt_file = create_rgb_psbt(
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
        input_tweak,
        &tx_resolver,
    )?;

    let psbt = PsbtResponse {
        psbt: psbt::serialize::Serialize::serialize(&psbt_file).to_hex(),
    };
    // TODO: Push to Carbonado (?)
    Ok(psbt)
}

pub async fn pay_asset(request: RgbTransferRequest) -> Result<RgbTransferResponse> {
    let RgbTransferRequest { rgb_invoice, psbt } = request;

    // TODO: Pull from Carbonado
    let mut stock = Stock::default();
    let (psbt, transfer) = pay_invoice(rgb_invoice, psbt, &mut stock)?;

    let commit = extract_commit(psbt.clone())?;
    let psbt = psbt.to_string();
    let consig = RgbTransferResponse {
        consig_id: transfer.bindle_id().to_string(),
        consig: transfer
            .to_strict_serialized::<0xFFFFFF>()
            .expect("invalid transfer serialization")
            .to_hex(),
        psbt,
        commit,
    };
    // TODO: Push to Carbonado
    Ok(consig)
}

pub async fn accept_transfer(request: AcceptRequest) -> Result<AcceptResponse> {
    let AcceptRequest { consignment } = request;

    // TODO: Pull from Carbonado
    let mut stock = Stock::default();

    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let mut tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };
    let resp = match accept_payment(consignment, true, &mut tx_resolver, &mut stock) {
        Ok(transfer) => AcceptResponse {
            contract_id: transfer.contract_id().to_string(),
            transfer_id: transfer.transfer_id().to_string(),
            valid: true,
        },
        Err((transfer, _)) => AcceptResponse {
            contract_id: transfer.contract_id().to_string(),
            transfer_id: transfer.transfer_id().to_string(),
            valid: false,
        },
    };

    // TODO: Push to Carbonado
    Ok(resp)
}

pub async fn list_contracts() -> Result<ContractsResponse> {
    let mut stock = Stock::default();

    let mut contracts = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            for contract_id in stock.contract_ids().expect("invalid contracts state") {
                if stock.contract_iface(contract_id, iface_id).is_ok() {
                    let face = stock.iface_by_id(iface_id).expect("invalid iface state");
                    let item = ContractDetail {
                        contract_id: contract_id.to_string(),
                        iface: face.name.to_string(),
                    };
                    contracts.push(item)
                }
            }
        }
    }

    Ok(ContractsResponse { contracts })
}

pub async fn list_interfaces() -> Result<InterfacesResponse> {
    let stock = Stock::default();

    let mut interfaces = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, iimpl) in schema.clone().iimpls.into_iter() {
            let face = stock.iface_by_id(iface_id).expect("invalid iface state");

            let item = InterfaceDetail {
                name: face.name.to_string(),
                iface: iface_id.to_string(),
                iimpl: iimpl.impl_id().to_string(),
            };
            interfaces.push(item)
        }
    }
    Ok(InterfacesResponse { interfaces })
}

pub async fn list_schemas() -> Result<SchemasResponse> {
    let stock = Stock::default();

    let mut schemas = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        let mut ifaces = vec![];
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            let face = stock.iface_by_id(iface_id).expect("invalid iface state");
            ifaces.push(face.name.to_string());
        }
        schemas.push(SchemaDetail {
            schema: schema_id.to_string(),
            ifaces,
        })
    }
    Ok(SchemasResponse { schemas })
}
