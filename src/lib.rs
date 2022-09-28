#[macro_use]
#[cfg(not(target_arch = "wasm32"))]
extern crate amplify;

use std::str::FromStr;

#[cfg(target_arch = "wasm32")]
use anyhow::anyhow;
use anyhow::{anyhow, Result};
use bdk::{wallet::AddressIndex, BlockTime, FeeRate, LocalUtxo};
#[cfg(not(target_arch = "wasm32"))]
use bitcoin::consensus::serialize as serialize_psbt;
use bitcoin::{
    consensus::deserialize as deserialize_psbt, psbt::PartiallySignedTransaction,
    util::address::Address, OutPoint, Transaction, Txid,
};
use bitcoin_hashes::{sha256, Hash};
use serde::{Deserialize, Serialize};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use tokio::{
    time::{sleep, Duration},
    try_join,
};

pub mod data;
mod operations;
mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(not(target_arch = "wasm32"))]
use crate::data::structs::{AssetResponse, ThinAsset};
pub use crate::data::{
    constants,
    structs::{
        FundVaultDetails, SatsInvoice, TransferRequest, TransferResponse, TransferResult, VaultData,
    },
};
use crate::operations::bitcoin::{
    create_transaction, dust_tx, get_wallet, new_mnemonic, save_mnemonic, sign_psbt,
    synchronize_wallet,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::operations::rgb::{
    /* accept_transfer, */ blind_utxo, get_asset_by_genesis, get_assets, issue_asset,
    transfer_asset, validate_transfer,
};
#[cfg(target_arch = "wasm32")]
use crate::util::post_json;

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
    let encrypted_message = vault_data.encrypt(&SharedKey::from_array(shared_key))?;
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
    let encrypted_message = vault_data.encrypt(&SharedKey::from_array(shared_key))?;
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
    let address = wallet.get_address(AddressIndex::LastUnused)?.to_string();
    info!("address:", &address);
    let balance = wallet.get_balance()?.to_string();
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
    fee_rate: Option<f32>,
) -> Result<Transaction> {
    let address = Address::from_str(address)?;

    let wallet = get_wallet(descriptor, Some(change_descriptor.to_owned()))?;
    synchronize_wallet(&wallet).await?;

    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let transaction =
        create_transaction(vec![SatsInvoice { address, amount }], &wallet, fee_rate).await?;

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
        assets_output: Some(outputs[0].to_owned()),
        assets_change_output: Some(outputs[1].to_owned()),
        udas_output: Some(outputs[2].to_owned()),
        udas_change_output: Some(outputs[3].to_owned()),
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

    let assets_output = assets_utxos.get(0).map(utxo_string);
    let assets_change_output = assets_utxos.get(1).map(utxo_string);
    let udas_output = uda_utxos.get(0).map(utxo_string);
    let udas_change_output = uda_utxos.get(1).map(utxo_string);

    Ok(FundVaultDetails {
        assets_output,
        assets_change_output,
        udas_output,
        udas_change_output,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn try_transfer(
    btc_descriptor_xprv: &str,
    btc_change_descriptor_xpub: &str,
    rgb_assets_descriptor_xprv: &str,
    rgb_assets_descriptor_xpub: &str,
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str,
    fee_rate: f32,
) -> Result<(String, String, String)> {
    let mut tries = 0;

    while tries < 10 {
        let btc_wallet = get_wallet(
            btc_descriptor_xprv,
            Some(btc_change_descriptor_xpub.to_owned()),
        )?;
        let assets_wallet = get_wallet(rgb_assets_descriptor_xprv, None)?;
        info!("Sync wallets");
        try_join!(
            synchronize_wallet(&assets_wallet),
            synchronize_wallet(&btc_wallet)
        )?;
        info!("Wallets synced");

        // Get a list of UTXOs in the assets wallet
        let asset_utxos = assets_wallet.list_unspent()?;
        info!(format!(
            "Found {} UTXOs in the assets wallet",
            asset_utxos.len()
        ));

        // Create a new tx for the change output, to be bundled
        let _dust_psbt = dust_tx(&btc_wallet, fee_rate, asset_utxos.get(0))?;
        info!("Created dust PSBT");
        info!("Creating transfer PSBT...");

        #[cfg(not(target_arch = "wasm32"))]
        let result = transfer_assets(
            rgb_assets_descriptor_xpub,
            blinded_utxo,
            amount,
            asset_contract,
            asset_utxos,
        )
        .await;

        #[cfg(target_arch = "wasm32")]
        let result = async {
            let endpoint = &constants::SEND_ASSETS_ENDPOINT;
            let body = TransferRequest {
                rgb_assets_descriptor_xpub: rgb_assets_descriptor_xpub.to_owned(),
                blinded_utxo: blinded_utxo.to_owned(),
                amount,
                asset_contract: asset_contract.to_owned(),
                asset_utxos,
            };
            let (transfer_res, status) = post_json(endpoint, &body).await?;
            if status != 200 {
                return Err(anyhow!("Error calling {}", endpoint.as_str()));
            }
            let TransferResponse {
                consignment,
                psbt,
                disclosure,
            } = serde_json::from_str(&transfer_res)?;
            Ok((consignment, psbt, disclosure))
        }
        .await;

        match result {
            Ok(result) => return Ok(result),
            Err(e) => {
                tries += 1;
                warn!(format!("Error: {e}. Try #{tries}..."));
                sleep(Duration::from_millis(500)).await;
            }
        }
    }

    Err(anyhow!("Error in try_transfer... Exceeded retries limit."))
}

#[allow(clippy::too_many_arguments)]
pub async fn send_assets(
    btc_descriptor_xprv: &str,
    btc_change_descriptor_xpub: &str,
    rgb_assets_descriptor_xprv: &str,
    rgb_assets_descriptor_xpub: &str,
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str,
    fee_rate: f32,
) -> Result<TransferResult> {
    let (consignment, psbt, disclosure) = try_transfer(
        btc_descriptor_xprv,
        btc_change_descriptor_xpub,
        rgb_assets_descriptor_xprv,
        rgb_assets_descriptor_xpub,
        blinded_utxo,
        amount,
        asset_contract,
        fee_rate,
    )
    .await?;

    info!("Successfully created assets PSBT");
    let psbt = base64::decode(&psbt)?;
    let psbt: PartiallySignedTransaction = deserialize_psbt(&psbt)?;

    info!("Signing and broadcasting transactions...");
    let assets_wallet = get_wallet(rgb_assets_descriptor_xprv, None)?;
    let tx = sign_psbt(&assets_wallet, psbt).await?;
    let txid = tx.txid().to_string();
    info!(format!("transfer txid was {txid}"));

    // let dust_tx = sign_psbt(&btc_wallet, dust_psbt).await?;
    // let dust_txid = dust_tx.txid().to_string();
    // info!(format!("dust txid was {dust_txid}"));

    Ok(TransferResult {
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
    String, // base64 sten consignment
    String, // base64 bitcoin encoded psbt
    String, // json
)> {
    let (consignment, psbt, disclosure) = transfer_asset(
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

    Ok((consignment, psbt, disclosure))
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
