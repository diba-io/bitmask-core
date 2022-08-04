#![allow(clippy::unused_unit)]
use std::str::FromStr;

use anyhow::{format_err, Result};
use bdk::{wallet::AddressIndex::LastUnused, BlockTime, TransactionDetails};
use bitcoin::util::address::Address;
use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use sha2::{Digest, Sha256};

mod data;
mod operations;
mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

use data::{
    constants,
    structs::{Asset, OutPoint, SatsInvoice, ThinAsset, TransferResponse},
};

use operations::{
    bitcoin::{create_transaction, get_mnemonic, get_wallet, save_mnemonic},
    rgb::{accept_transfer, blind_utxo, get_asset, get_assets, transfer_asset, validate_transfer},
};

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

pub fn get_vault(password: String, encrypted_descriptors: String) -> Result<VaultData> {
    let mut hasher = Sha256::new();

    // write input message
    hasher.update(password.as_bytes());

    // read hash digest and consume hasher
    let result = hasher.finalize();
    let shared_key: [u8; 32] = result
        .as_slice()
        .try_into()
        .expect("slice with incorrect length");
    let encrypted_descriptors: Vec<u8> = serde_json::from_str(&encrypted_descriptors).unwrap();
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
    encryption_password: String,
    seed_password: String,
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
    ) = get_mnemonic(&seed_password);
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
    mnemonic: String,
    encryption_password: String,
    seed_password: String,
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
    ) = save_mnemonic(&seed_password, mnemonic.clone());
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
    descriptor: String,
    change_descriptor: Option<String>,
) -> Result<WalletData> {
    log!("get_wallet_data");
    log!(&descriptor, format!("{:?}", &change_descriptor));

    let wallet = get_wallet(descriptor, change_descriptor).await;
    let address = wallet
        .as_ref()
        .unwrap()
        .get_address(LastUnused)
        .unwrap()
        .to_string();
    log!(&address);
    let balance = wallet.as_ref().unwrap().get_balance().unwrap().to_string();
    log!(&balance);
    let unspent = wallet.as_ref().unwrap().list_unspent().unwrap_or_default();
    let unspent: Vec<String> = unspent
        .into_iter()
        .map(|x| x.outpoint.to_string())
        .collect();
    log!(format!("unspent: {unspent:#?}"));

    let transactions = wallet
        .as_ref()
        .unwrap()
        .list_transactions(false)
        .unwrap_or_default();
    log!(format!("transactions: {transactions:#?}"));

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

pub async fn import_list_assets(node_url: Option<String>) -> Result<Vec<Asset>> {
    log!("import_list_assets");
    let assets = get_assets(node_url).await?;
    log!(format!("get assets: {assets:#?}"));
    Ok(assets)
}

pub async fn import_asset(
    rgb_tokens_descriptor: String,
    asset: Option<String>,
    genesis: Option<String>,
    node_url: Option<String>,
) -> Result<ThinAsset> {
    let wallet = get_wallet(rgb_tokens_descriptor, None).await;
    let unspent = wallet.as_ref().unwrap().list_unspent().unwrap_or_default();
    log!(format!("asset: {asset:#?}\tgenesis: {genesis:#?}"));
    match asset {
        Some(asset) => {
            let asset = get_asset(Some(asset), None, unspent, node_url).await;
            log!(format!("get asset {asset:#?}"));
            match asset {
                Ok(asset) => Ok(asset),
                Err(e) => Err(format_err!("Server error: {e}")),
            }
        }
        None => {
            log!("genesis....");
            match genesis {
                Some(_genesis) => todo!("Import asset from genesis not yet implemented"),
                None => Err(format_err!("Error: Unknown error in import_asset")),
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct TransactionData {
    blinding: String,
    utxo: OutPoint,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlindingUtxo {
    conceal: String,
    blinding: String,
    utxo: OutPoint,
}

pub async fn set_blinded_utxo(
    utxo_string: String,
    node_url: Option<String>,
) -> Result<BlindingUtxo> {
    let mut split = utxo_string.split(':');
    let utxo = OutPoint {
        txid: split.next().unwrap().to_string(),
        vout: split.next().unwrap().to_string().parse::<u32>().unwrap(),
    };
    let (blind, utxo) = blind_utxo(utxo, node_url).await?;

    let blinding_utxo = BlindingUtxo {
        conceal: blind.conceal,
        blinding: blind.blinding,
        utxo,
    };

    Ok(blinding_utxo)
}

pub async fn send_sats(
    descriptor: String,
    change_descriptor: String,
    address: String,
    amount: u64,
) -> Result<TransactionDetails> {
    let address = Address::from_str(&(address));

    let wallet = get_wallet(descriptor, Some(change_descriptor))
        .await
        .unwrap();

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

pub async fn fund_wallet(
    descriptor: String,
    change_descriptor: String,
    address: String,
    uda_address: String,
) -> Result<TransactionDetails> {
    let address = Address::from_str(&(address));
    let uda_address = Address::from_str(&(uda_address));

    let wallet = get_wallet(descriptor, Some(change_descriptor))
        .await
        .unwrap();
    let invoice = SatsInvoice {
        address: address.unwrap(),
        amount: 2000,
    };
    let uda_invoice = SatsInvoice {
        address: uda_address.unwrap(),
        amount: 2000,
    };
    let transaction = create_transaction(
        vec![invoice.clone(), invoice, uda_invoice.clone(), uda_invoice],
        &wallet,
    )
    .await?;

    Ok(transaction)
}

pub async fn send_tokens(
    btc_descriptor: String,
    btc_change_descriptor: String,
    rgb_tokens_descriptor: String,
    blinded_utxo: String,
    amount: u64,
    asset: ThinAsset,
    node_url: Option<String>,
) -> Result<TransferResponse> {
    let assets_wallet = get_wallet(rgb_tokens_descriptor.clone(), None)
        .await
        .unwrap();
    let full_wallet = get_wallet(rgb_tokens_descriptor.clone(), Some(btc_descriptor))
        .await
        .unwrap();
    let full_change_wallet = get_wallet(rgb_tokens_descriptor, Some(btc_change_descriptor))
        .await
        .unwrap();
    let consignment = transfer_asset(
        blinded_utxo,
        amount,
        asset,
        &full_wallet,
        &full_change_wallet,
        &assets_wallet,
        node_url,
    )
    .await?;

    Ok(consignment)
}

pub async fn validate_transaction(consignment: String, node_url: Option<String>) -> Result<()> {
    validate_transfer(consignment, node_url).await
}

pub async fn accept_transaction(
    consignment: String,
    txid: String,
    vout: u32,
    blinding: String,
    node_url: Option<String>,
) -> Result<String> {
    let transaction_data = TransactionData {
        blinding,
        utxo: OutPoint { txid, vout },
    };
    let accept = accept_transfer(
        consignment,
        transaction_data.utxo,
        transaction_data.blinding,
        node_url,
    )
    .await?;
    log!("hola denueveo 3");
    Ok(accept)
}

pub async fn import_accept(
    rgb_tokens_descriptor: String,
    asset: String,
    consignment: String,
    txid: String,
    vout: u32,
    blinding: String,
    node_url: Option<String>,
) -> Result<ThinAsset> {
    let transaction_data = TransactionData {
        blinding,
        utxo: OutPoint { txid, vout },
    };

    let accept = accept_transfer(
        consignment,
        transaction_data.utxo,
        transaction_data.blinding,
        node_url.clone(),
    )
    .await;
    match accept {
        Ok(_accept) => {
            let wallet = get_wallet(rgb_tokens_descriptor, None).await;
            let unspent = wallet.as_ref().unwrap().list_unspent().unwrap_or_default();
            let asset = get_asset(Some(asset), None, unspent, node_url).await;
            log!(format!("get asset {asset:#?}"));
            asset
        }
        Err(e) => Err(e),
    }
}

pub fn switch_network(network_str: &str) {
    constants::switch_network(network_str);
}
