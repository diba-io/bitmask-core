#[macro_use]
extern crate amplify;

use std::str::FromStr;

use anyhow::{format_err, Result};
use bdk::{wallet::AddressIndex::LastUnused, BlockTime};
use bitcoin::{util::address::Address, OutPoint, Transaction, Txid};
use operations::rgb::ConsignmentDetails;
use serde::{Deserialize, Serialize};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use sha2::{Digest, Sha256};

pub mod data;
mod operations;
mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

use data::{
    constants,
    structs::{AssetResponse, SatsInvoice, ThinAsset, TransferResponse},
};

use operations::{
    bitcoin::{create_transaction, get_mnemonic, get_wallet, save_mnemonic},
    rgb::{
        accept_transfer, blind_utxo, get_asset_by_contract_id, get_asset_by_genesis, get_assets,
        issue_asset, /* rgb_address, */ transfer_asset, validate_transfer,
    },
};

use crate::operations::bitcoin::synchronize_wallet;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VaultData {
    pub btc_descriptor: String,
    pub btc_change_descriptor: String,
    pub rgb_tokens_descriptor: String,
    pub rgb_nfts_descriptor: String,
    pub pubkey_hash: String,
}

impl SerdeEncryptSharedKey for VaultData {
    type S = BincodeSerializer<Self>; // you can specify serializer implementation (or implement it by yourself).
}

pub fn get_vault(password: &str, encrypted_descriptors: &str) -> Result<VaultData> {
    let mut hasher = Sha256::new();

    // write input message
    hasher.update(password.as_bytes());

    // read hash digest and consume hasher
    let result = hasher.finalize();
    let shared_key: [u8; 32] = result
        .as_slice()
        .try_into()
        .expect("slice with incorrect length");
    let encrypted_descriptors: Vec<u8> = serde_json::from_str(encrypted_descriptors).unwrap();
    // STORAGE_KEY_DESCRIPTOR_ENCRYPTED
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors);
    match encrypted_message {
        Ok(encrypted_message) => {
            let vault_data =
                VaultData::decrypt_owned(&encrypted_message, &SharedKey::from_array(shared_key));
            match vault_data {
                Ok(vault_data) => Ok(vault_data),
                Err(e) => Err(format_err!("Error: {e}")),
            }
        }
        Err(e) => Err(format_err!("Error: {e}")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MnemonicSeedData {
    pub mnemonic: String,
    pub serialized_encrypted_message: Vec<u8>,
}

pub fn get_mnemonic_seed(
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let mut hasher = Sha256::new();

    // write input message
    hasher.update(encryption_password.as_bytes());

    // read hash digest and consume hasher
    let hash = hasher.finalize();
    let shared_key: [u8; 32] = hash
        .as_slice()
        .try_into()
        .expect("slice with incorrect length");

    let (
        mnemonic,
        btc_descriptor,
        btc_change_descriptor,
        rgb_tokens_descriptor,
        rgb_nfts_descriptor,
        pubkey_hash,
    ) = get_mnemonic(seed_password);
    let vault_data = VaultData {
        btc_descriptor,
        btc_change_descriptor,
        rgb_tokens_descriptor,
        rgb_nfts_descriptor,
        pubkey_hash,
    };
    let encrypted_message = vault_data
        .encrypt(&SharedKey::from_array(shared_key))
        .unwrap();
    let serialized_encrypted_message: Vec<u8> = encrypted_message.serialize();
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub fn save_mnemonic_seed(
    mnemonic: &str,
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let mut hasher = Sha256::new();

    // write input message
    hasher.update(encryption_password.as_bytes());

    // read hash digest and consume hasher
    let hash = hasher.finalize();
    let shared_key: [u8; 32] = hash
        .as_slice()
        .try_into()
        .expect("slice with incorrect length");

    let (
        btc_descriptor,
        btc_change_descriptor,
        rgb_tokens_descriptor,
        rgb_nfts_descriptor,
        pubkey_hash,
    ) = save_mnemonic(seed_password, mnemonic);
    let vault_data = VaultData {
        btc_descriptor,
        btc_change_descriptor,
        rgb_tokens_descriptor,
        rgb_nfts_descriptor,
        pubkey_hash,
    };
    let encrypted_message = vault_data
        .encrypt(&SharedKey::from_array(shared_key))
        .unwrap();
    let serialized_encrypted_message: Vec<u8> = encrypted_message.serialize();
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: mnemonic.to_owned(),
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
    change_descriptor: Option<&str>,
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

// pub fn get_rgb_address(descriptor_str: &str, index: u16, change: bool) -> Result<String> {
//     rgb_address(descriptor_str, index, change)
// }

pub async fn import_list_assets(node_url: Option<String>) -> Result<Vec<AssetResponse>> {
    info!("import_list_assets");
    let assets = get_assets(node_url).await?;
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

pub async fn import_asset(
    rgb_tokens_descriptor: Option<&str>,
    contract_id: Option<&str>,
    genesis: Option<&str>,
    node_url: Option<String>,
) -> Result<ThinAsset> {
    match genesis {
        Some(genesis) => {
            info!("Getting asset by genesis:", genesis);
            get_asset_by_genesis(genesis)
        }
        None => match (contract_id, rgb_tokens_descriptor) {
            (Some(contract_id), Some(rgb_tokens_descriptor)) => {
                info!("Getting asset by contract id:", contract_id);
                let wallet = get_wallet(rgb_tokens_descriptor, None)?;
                let unspent = wallet.list_unspent().unwrap_or_default();
                let asset = get_asset_by_contract_id(contract_id, unspent, node_url).await;
                info!(format!("asset: {asset:?}"));
                match asset {
                    Ok(asset) => Ok(asset),
                    Err(e) => Err(format_err!("Server error: {e}")),
                }
            }
            _ => Err(format_err!("Error: Unknown error in import_asset")),
        },
    }
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

pub async fn send_sats(
    descriptor: &str,
    change_descriptor: &str,
    address: String,
    amount: u64,
) -> Result<Transaction> {
    let address = Address::from_str(&(address));

    let wallet = get_wallet(descriptor, Some(change_descriptor))?;

    let transaction = create_transaction(
        vec![SatsInvoice {
            address: address.unwrap(),
            amount,
        }],
        &wallet,
    )
    .await?;

    Ok(transaction)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FundVaultDetails {
    pub txid: String,
    pub send_assets: String,
    pub recv_assets: String,
    pub send_udas: String,
    pub recv_udas: String,
}

pub async fn fund_wallet(
    descriptor: &str,
    change_descriptor: &str,
    address: &str,
    uda_address: &str,
) -> Result<FundVaultDetails> {
    let address = Address::from_str(address);
    let uda_address = Address::from_str(uda_address);

    let wallet = get_wallet(descriptor, Some(change_descriptor))?;

    let asset_invoice = SatsInvoice {
        address: address.unwrap(),
        amount: 613,
    };
    let uda_invoice = SatsInvoice {
        address: uda_address.unwrap(),
        amount: 613,
    };

    let details = create_transaction(
        vec![
            asset_invoice.clone(),
            asset_invoice,
            uda_invoice.clone(),
            uda_invoice,
        ],
        &wallet,
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
        send_assets: outputs[0].clone(),
        recv_assets: outputs[1].clone(),
        send_udas: outputs[2].clone(),
        recv_udas: outputs[3].clone(),
    })
}

pub async fn send_tokens(
    btc_descriptor: &str,
    // btc_change_descriptor: &str,
    rgb_tokens_descriptor: &str,
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str,
) -> Result<(ConsignmentDetails, Transaction, TransferResponse)> {
    let full_wallet = get_wallet(rgb_tokens_descriptor, Some(btc_descriptor))?;
    // let full_change_wallet = get_wallet(rgb_tokens_descriptor, Some(btc_change_descriptor))?;
    let assets_wallet = get_wallet(rgb_tokens_descriptor, None)?;
    let (consignment, tx, response) = transfer_asset(
        blinded_utxo,
        amount,
        asset_contract,
        &full_wallet,
        // &full_change_wallet,
        &assets_wallet,
        rgb_tokens_descriptor,
    )
    .await?;

    Ok((consignment, tx, response))
}

pub async fn validate_transaction(consignment: &str, node_url: Option<String>) -> Result<()> {
    validate_transfer(consignment.to_owned(), node_url).await
}

pub async fn accept_transaction(
    consignment: &str,
    txid: &str,
    vout: u32,
    blinding: &str,
    node_url: Option<String>,
) -> Result<String> {
    let txid = Txid::from_str(txid)?;

    let transaction_data = TransactionData {
        blinding: blinding.to_owned(),
        utxo: OutPoint { txid, vout },
    };
    let accept = accept_transfer(
        consignment.to_owned(),
        transaction_data.utxo,
        transaction_data.blinding,
        node_url,
    )
    .await?;
    info!("Transaction accepted");
    Ok(accept)
}

pub async fn import_accept(
    rgb_tokens_descriptor: &str,
    asset: &str,
    consignment: &str,
    txid: &str,
    vout: u32,
    blinding: String,
    node_url: Option<String>,
) -> Result<ThinAsset> {
    let txid = Txid::from_str(txid)?;

    let transaction_data = TransactionData {
        blinding,
        utxo: OutPoint { txid, vout },
    };

    let accept = accept_transfer(
        consignment.to_owned(),
        transaction_data.utxo,
        transaction_data.blinding,
        node_url.clone(),
    )
    .await;
    match accept {
        Ok(_accept) => {
            let asset =
                import_asset(Some(rgb_tokens_descriptor), Some(asset), None, node_url).await;
            info!(format!("get asset {asset:#?}"));
            asset
        }
        Err(e) => Err(e),
    }
}

pub async fn switch_network(network_str: &str) -> Result<()> {
    constants::switch_network(network_str).await
}

pub fn get_network() -> Result<String> {
    match constants::NETWORK.read() {
        Ok(network) => Ok(network.to_string()),
        Err(err) => Ok(err.to_string()),
    }
}
