#[macro_use]
#[cfg(not(target_arch = "wasm32"))]
extern crate amplify;

use std::str::FromStr;

use anyhow::{anyhow, Result};
use bdk::{wallet::AddressIndex::LastUnused, BlockTime, FeeRate, LocalUtxo};
use bitcoin::{
    consensus::{deserialize as deserialize_psbt, serialize as serialize_psbt},
    util::address::Address,
    OutPoint, Transaction, Txid,
};
use bitcoin_hashes::{sha256, Hash};
#[cfg(not(target_arch = "wasm32"))]
use operations::rgb::ConsignmentDetails;
use serde::{Deserialize, Serialize};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};

pub mod data;
mod operations;
mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(not(target_arch = "wasm32"))]
use data::structs::{
    AssetResponse, ThinAsset, TransferRequestExt, TransferResponse, TransferResponseExt,
};
pub use data::{
    constants,
    structs::{FundVaultDetails, SatsInvoice, VaultData},
};
use operations::bitcoin::{
    create_transaction, get_wallet, new_mnemonic, save_mnemonic, sign_psbt, synchronize_wallet,
};
#[cfg(not(target_arch = "wasm32"))]
use operations::rgb::{
    /* accept_transfer, */ blind_utxo, get_asset_by_genesis, get_assets, issue_asset,
    transfer_asset, validate_transfer,
};
#[cfg(not(target_arch = "wasm32"))]
use util::post_json;

impl SerdeEncryptSharedKey for VaultData {
    type S = BincodeSerializer<Self>; // you can specify serializer implementation (or implement it by yourself).
}

pub fn get_vault(password: &str, encrypted_descriptors: &str) -> Result<VaultData> {
    // read hash digest and consume hasher
    let hash = sha256::Hash::hash(password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();
    let encrypted_descriptors: Vec<u8> = hex::decode(encrypted_descriptors)?;
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors)?;
    Ok(VaultData::decrypt_owned(
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

pub fn get_mnemonic_seed(
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let vault_data = new_mnemonic(seed_password)?;
    let encrypted_message = vault_data
        .encrypt(&SharedKey::from_array(shared_key))
        .unwrap();
    let serialized_encrypted_message = hex::encode(&encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: vault_data.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub fn save_mnemonic_seed(
    mnemonic: &str,
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let vault_data = save_mnemonic(seed_password, mnemonic)?;
    let encrypted_message = vault_data
        .encrypt(&SharedKey::from_array(shared_key))
        .unwrap();
    let serialized_encrypted_message = hex::encode(&encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: vault_data.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

#[derive(Serialize, Deserialize)]
pub struct WalletData {
    pub address: String,
    pub balance: String,
    pub transactions: Vec<WalletTransaction>,
    pub unspent: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct WalletTransaction {
    pub txid: Txid,
    pub received: u64,
    pub sent: u64,
    pub fee: Option<u64>,
    pub confirmed: bool,
    pub confirmation_time: Option<BlockTime>,
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
    let address = wallet.get_address(LastUnused).unwrap().to_string();
    info!("address:", &address);
    let balance = wallet.get_balance().unwrap().to_string();
    info!("balance:", &balance);
    let unspent = wallet.list_unspent().unwrap_or_default();
    let unspent: Vec<String> = unspent
        .into_iter()
        .map(|x| x.outpoint.to_string())
        .collect();
    trace!(format!("unspent: {unspent:#?}"));

    let transactions = wallet.list_transactions(false).unwrap_or_default();
    trace!(format!("transactions: {transactions:#?}"));

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
        unspent,
    })
}

// TODO: this fn is only needed on desktop wallet
#[cfg(not(target_arch = "wasm32"))]
pub fn list_assets(contract: &str) -> Result<Vec<AssetResponse>> {
    info!("list_assets");
    let assets = get_assets(contract)?;
    info!(format!("get assets: {assets:#?}"));
    Ok(assets)
}

#[derive(Serialize, Deserialize)]
pub struct CreateAssetResult {
    pub genesis: String,   // in bech32m encoding
    pub id: String,        // consignment ID
    pub asset_id: String,  // consignment ID
    pub schema_id: String, // consignment ID
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_asset(
    ticker: &str,
    name: &str,
    precision: u8,
    supply: u64,
    utxo: &str,
) -> Result<CreateAssetResult> {
    let utxo = OutPoint::from_str(utxo)?;
    let contract = issue_asset(ticker, name, precision, supply, utxo)?;
    let genesis = contract.to_string();
    let id = contract.id().to_string();
    let asset_id = contract.contract_id().to_string();
    let schema_id = contract.schema_id().to_string();
    Ok(CreateAssetResult {
        genesis,
        id,
        asset_id,
        schema_id,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn import_asset(genesis: &str) -> Result<ThinAsset> {
    info!("Getting asset by genesis:", genesis);
    get_asset_by_genesis(genesis)
}

#[derive(Serialize, Deserialize)]
struct TransactionData {
    blinding: String,
    utxo: OutPoint,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindingUtxo {
    pub conceal: String,
    pub blinding: String,
    pub utxo: OutPoint,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_blinded_utxo(utxo_string: &str) -> Result<BlindingUtxo> {
    let mut split = utxo_string.split(':');
    let utxo = OutPoint {
        txid: Txid::from_str(split.next().unwrap())?,
        vout: split.next().unwrap().to_string().parse::<u32>()?,
    };

    let (blind, utxo) = blind_utxo(utxo)?;

    let blinding_utxo = BlindingUtxo {
        conceal: blind.conceal,
        blinding: blind.blinding,
        utxo,
    };

    Ok(blinding_utxo)
}

// pub fn get_blinded_utxo(rgb_descriptor: &str) -> Result<BlindingUtxo> {
//     let rgb_wallet = get_wallet(rgb_descriptor, None)?;

//     // ensure there's always a receive utxo

//     let (blind, utxo) = blind_utxo(utxo)?;

//     let blinding_utxo = BlindingUtxo {
//         conceal: blind.conceal,
//         blinding: blind.blinding,
//         utxo,
//     };

//     Ok(blinding_utxo)
// }

pub async fn send_sats(
    descriptor: &str,
    change_descriptor: &str,
    address: &str,
    amount: u64,
    fee_rate: Option<FeeRate>,
) -> Result<Transaction> {
    let address = Address::from_str(address);

    let wallet = get_wallet(descriptor, Some(change_descriptor.to_owned()))?;
    synchronize_wallet(&wallet).await?;

    let transaction = create_transaction(
        vec![SatsInvoice {
            address: address.unwrap(),
            amount,
        }],
        &wallet,
        fee_rate,
    )
    .await?;

    Ok(transaction)
}

pub async fn fund_wallet(
    descriptor: &str,
    change_descriptor: &str,
    address: &str,
    uda_address: &str,
    fee_rate: Option<FeeRate>,
    asset_amount: u64,
    uda_amount: u64,
) -> Result<FundVaultDetails> {
    let address = Address::from_str(address);
    let uda_address = Address::from_str(uda_address);

    let wallet = get_wallet(descriptor, Some(change_descriptor.to_owned()))?;
    synchronize_wallet(&wallet).await?;

    let asset_invoice = SatsInvoice {
        address: address.unwrap(),
        amount: asset_amount,
    };
    let uda_invoice = SatsInvoice {
        address: uda_address.unwrap(),
        amount: uda_amount,
    };

    let details = create_transaction(
        vec![
            asset_invoice.clone(),
            asset_invoice,
            uda_invoice.clone(),
            uda_invoice,
        ],
        &wallet,
        fee_rate,
    )
    .await?;

    let txid = details.txid();
    let outputs: Vec<String> = details
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{txid}:{i}"))
        .collect();

    Ok(FundVaultDetails {
        txid: txid.to_string(),
        assets: outputs[0].clone(),
        assets_change: outputs[1].clone(),
        udas: outputs[2].clone(),
        udas_change: outputs[3].clone(),
    })
}

pub async fn get_assets_vault(assets_descriptor: &str) -> Result<FundVaultDetails> {
    let assets_wallet = get_wallet(assets_descriptor, None)?;
    synchronize_wallet(&assets_wallet).await?;

    let asset_utxos = assets_wallet.list_unspent()?;

    debug!(format!("Asset UTXOs: {asset_utxos:#?}"));

    match asset_utxos.get(0) {
        Some(asset_utxo) => {
            let txid = asset_utxo.outpoint.txid.to_string();
            let output = asset_utxo.outpoint.to_string();

            Ok(FundVaultDetails {
                txid,
                assets: output.clone(), // TODO: Make it work with other UTXOs
                assets_change: output.clone(),
                udas: output.clone(),
                udas_change: output,
            })
        }
        None => Err(anyhow!("No asset UTXOs")),
    }
}

pub async fn send_assets(
    rgb_assets_descriptor_xprv: &str,
    rgb_assets_descriptor_xpub: &str,
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str,
) -> Result<TransferResponse> {
    let assets_wallet = get_wallet(rgb_assets_descriptor_xprv, None)?;
    debug!("sync wallet");
    synchronize_wallet(&assets_wallet).await?;
    debug!("wallet synced");

    let asset_utxos = assets_wallet.list_unspent()?;

    // #[cfg(not(target_arch = "wasm32"))]
    // let (_, consignment, psbt, disclosure) = transfer_assets(
    //     rgb_assets_descriptor_xpub,
    //     blinded_utxo,
    //     amount,
    //     asset_contract,
    //     asset_utxos,
    // )
    // .await?;
    #[cfg(not(target_arch = "wasm32"))]
    let (consignment, psbt, disclosure) = async {
        let endpoint = &constants::SEND_ASSETS_ENDPOINT;
        let body = TransferRequestExt {
            rgb_assets_descriptor_xpub: rgb_assets_descriptor_xpub.to_string(),
            blinded_utxo: blinded_utxo.to_string(),
            amount,
            asset_contract: asset_contract.to_string(),
            asset_utxos,
        };
        let (transfer_res, status) = post_json(endpoint, &body).await?;
        if status != 200 {
            return Err(anyhow!("Error calling {}", endpoint.as_str()));
        }
        let TransferResponseExt {
            consignment,
            psbt,
            disclosure,
        } = serde_json::from_str(&transfer_res)?;
        let psbt = base64::decode(&psbt)?;
        let psbt = deserialize_psbt(&psbt)?;
        Ok((consignment, psbt, disclosure))
    }
    .await?;

    let tx = sign_psbt(&assets_wallet, psbt).await?;
    let txid = tx.txid().to_string();

    Ok(TransferResponse {
        consignment,
        disclosure,
        txid,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn transfer_assets(
    rgb_assets_descriptor_xpub: &str, // TODO: Privacy concerns. Not great, not terrible
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str,
    asset_utxos: Vec<LocalUtxo>,
) -> Result<(
    ConsignmentDetails,
    String, // base64 sten consignment
    String, // base64 bitcoin encoded psbt
    String, // json
)> {
    let (consignment_details, consignment, psbt, disclosure) = transfer_asset(
        rgb_assets_descriptor_xpub,
        blinded_utxo,
        amount,
        asset_contract,
        asset_utxos,
    )
    .await?;

    let consignment = base64::encode(&consignment);
    let psbt = serialize_psbt(&psbt);
    let psbt = base64::encode(&psbt);
    let disclosure = serde_json::to_string(&disclosure)?;

    Ok((consignment_details, consignment, psbt, disclosure))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn validate_transaction(consignment: &str) -> Result<()> {
    validate_transfer(consignment.to_owned()).await
}

// TODO: implement accept_transfer in RGB ops
// #[cfg(not(target_arch = "wasm32"))]
// pub async fn accept_transfer(
//     consignment: &str,
//     txid: &str,
//     vout: u32,
//     blinding: &str,
// ) -> Result<String> {
//     let txid = Txid::from_str(txid)?;

//     let transaction_data = TransactionData {
//         blinding: blinding.to_owned(),
//         utxo: OutPoint { txid, vout },
//     };
//     let accept = accept_transfer(
//         consignment,
//         transaction_data.utxo,
//         transaction_data.blinding,
//     )
//     .await?;
//     info!("Transaction accepted");
//     Ok(accept)
// }

pub async fn switch_network(network_str: &str) -> Result<()> {
    constants::switch_network(network_str).await
}

pub fn get_network() -> Result<String> {
    match constants::NETWORK.read() {
        Ok(network) => Ok(network.to_string()),
        Err(err) => Ok(err.to_string()),
    }
}
