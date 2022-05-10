#![allow(clippy::unused_unit)]
use std::collections::HashMap;

use bdk::{wallet::AddressIndex::LastUnused, BlockTime};
use bitcoin::Txid;
use gloo_console::log;
use js_sys::Promise;
use serde::{Deserialize, Serialize};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

mod data;
mod operations;
mod utils;

use data::{
    constants,
    structs::{OutPoint, ThinAsset},
};

use operations::{
    bitcoin::{create_transaction, get_mnemonic, get_wallet, save_mnemonic},
    rgb::{accept_transfer, blind_utxo, get_asset, get_assets, transfer_asset, validate_transfer},
};

pub use utils::{json_parse, resolve, set_panic_hook, to_string};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

trait FromString {
    fn from_string(str: String) -> JsValue;
}

impl FromString for JsValue {
    fn from_string(str: String) -> JsValue {
        JsValue::from_str(&str)
    }
}

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

#[wasm_bindgen]
pub fn get_vault(password: String, encrypted_descriptors: String) -> Promise {
    set_panic_hook();
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
                Ok(vault_data) => future_to_promise(async move {
                    Ok(JsValue::from_string(
                        serde_json::to_string(&vault_data).unwrap(),
                    ))
                }),
                Err(e) => {
                    future_to_promise(
                        async move { Ok(JsValue::from_string(format!("Error: {} ", e))) },
                    )
                }
            }
        }
        Err(e) => {
            future_to_promise(async move { Ok(JsValue::from_string(format!("Error: {} ", e))) })
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MnemonicSeedData {
    pub mnemonic: String,
    pub serialized_encrypted_message: Vec<u8>,
}

#[wasm_bindgen]
pub fn get_mnemonic_seed(encryption_password: String, seed_password: String) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
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

        Ok(JsValue::from_string(
            serde_json::to_string(&mnemonic_seed_data).unwrap(),
        ))
    })
}

#[wasm_bindgen]
pub fn save_mnemonic_seed(
    mnemonic: String,
    encryption_password: String,
    seed_password: String,
) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
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

        Ok(JsValue::from_string(
            serde_json::to_string(&mnemonic_seed_data).unwrap(),
        ))
    })
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

#[wasm_bindgen]
pub fn get_wallet_data(descriptor: String, change_descriptor: Option<String>) -> Promise {
    log!("get_wallet_data");
    log!(&descriptor, change_descriptor.as_ref());
    set_panic_hook();
    future_to_promise(async {
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

        let wallet_data = WalletData {
            address,
            balance,
            transactions,
            unspent,
        };
        let wallet_data = serde_json::to_string(&wallet_data).unwrap();
        Ok(JsValue::from_string(wallet_data))
    })
}

#[wasm_bindgen]
pub fn import_list_assets() -> Promise {
    set_panic_hook();
    log!("import_list_assets");
    future_to_promise(async {
        let assets = get_assets().await;
        log!(format!("get assets: {assets:#?}"));
        let assets = serde_json::to_string(&assets.unwrap());
        match assets {
            Ok(assets) => {
                log!(&assets);
                Ok(JsValue::from_string(assets))
            }
            Err(e) => Ok(JsValue::from_string(format!("Error: {} ", e))),
        }
    })
}

#[wasm_bindgen]
pub fn import_asset(
    rgb_tokens_descriptor: String,
    asset: Option<String>,
    genesis: Option<String>,
) -> Promise {
    set_panic_hook();
    future_to_promise(async {
        let wallet = get_wallet(rgb_tokens_descriptor, None).await;
        let unspent = wallet.as_ref().unwrap().list_unspent().unwrap_or_default();
        log!(format!("asset: {asset:#?}\tgenesis: {genesis:#?}"));
        match asset {
            Some(asset) => {
                let asset = get_asset(Some(asset), None, unspent).await;
                log!(format!("get asset {asset:#?}"));
                let asset = match asset {
                    Ok(asset) => asset,
                    Err(e) => return Ok(JsValue::from_string(format!("Server error: {} ", e))),
                };
                let asset = serde_json::to_string(&asset);
                match asset {
                    Ok(asset) => {
                        log!(&asset);
                        Ok(JsValue::from_string(asset))
                    }
                    Err(e) => Ok(JsValue::from_string(format!("Error: {} ", e))),
                }
            }
            None => {
                log!("genesis....");
                match genesis {
                    Some(genesis) => Ok(JsValue::from_string(format!(
                        "Error: genesis {} gives error ",
                        genesis
                    ))),
                    None => Ok(JsValue::from_str("Error generic")),
                }
            }
        }
    })
}

#[derive(Serialize, Deserialize)]
struct TransactionData {
    blinding: String,
    utxo: OutPoint,
}

#[derive(Serialize, Deserialize, Clone)]
struct BlindingUtxo {
    conceal: String,
    blinding: String,
    utxo: OutPoint,
}

#[wasm_bindgen]
pub fn set_blinded_utxo(utxo_string: String) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
        let mut split = utxo_string.split(':');
        let utxo = OutPoint {
            txid: split.next().unwrap().to_string(),
            vout: split.next().unwrap().to_string().parse::<u32>().unwrap(),
        };
        let (blind, utxo) = blind_utxo(utxo).await.unwrap(); // TODO: Error handling
        let blinding_utxo = BlindingUtxo {
            conceal: blind.conceal,
            blinding: blind.blinding,
            utxo,
        };
        Ok(JsValue::from_string(
            serde_json::to_string(&blinding_utxo).unwrap(),
        ))
    })
}

#[wasm_bindgen]
pub fn send_sats(
    descriptor: String,
    change_descriptor: String,
    address: String,
    amount: u64,
) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
        let wallet = get_wallet(descriptor, Some(change_descriptor))
            .await
            .unwrap();
        let transaction = create_transaction(address, amount, &wallet).await;
        match transaction {
            Ok(transaction) => Ok(JsValue::from_string(transaction)),
            Err(e) => Ok(JsValue::from_string(format!("{} ", e))),
        }
    })
}

#[wasm_bindgen]
pub fn send_tokens(
    rgb_tokens_descriptor: String,
    blinded_utxo: String,
    amount: u64,
    asset: String,
) -> Promise {
    set_panic_hook();
    let asset: ThinAsset = serde_json::from_str(&asset).unwrap();
    future_to_promise(async move {
        let wallet = get_wallet(rgb_tokens_descriptor, None).await.unwrap();
        let consignment = transfer_asset(blinded_utxo, amount, asset, &wallet).await;
        match consignment {
            Ok(consignment) => Ok(JsValue::from_string(consignment)),
            Err(e) => Ok(JsValue::from_string(format!("Error: {} ", e))),
        }
    })
}

#[wasm_bindgen]
pub fn validate_transaction(consignment: String) -> Promise {
    set_panic_hook();
    future_to_promise(async {
        let validate = validate_transfer(consignment).await.unwrap();
        Ok(JsValue::from_string(
            serde_json::to_string(&validate).unwrap(),
        ))
    })
}

#[wasm_bindgen]
pub fn accept_transaction(
    consignment: String,
    txid: String,
    vout: u32,
    blinding: String,
) -> Promise {
    set_panic_hook();
    log!("hola accept");
    let transaction_data = TransactionData {
        blinding,
        utxo: OutPoint { txid, vout },
    };
    log!("hola denueveo");
    future_to_promise(async move {
        log!("hola denueveo 2");
        let accept = accept_transfer(
            consignment,
            transaction_data.utxo,
            transaction_data.blinding,
        )
        .await;
        log!("hola denueveo 3");
        match accept {
            Ok(accept) => Ok(JsValue::from_string(
                serde_json::to_string(&accept).unwrap(),
            )),
            Err(e) => Ok(JsValue::from_string(format!("Error: {} ", e))),
        }
    })
}

#[wasm_bindgen]
pub fn switch_network(network_str: &str) {
    constants::switch_network(network_str);
}
