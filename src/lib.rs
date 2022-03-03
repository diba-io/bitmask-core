#![allow(clippy::unused_unit)]
use std::collections::HashMap;

use bdk::{wallet::AddressIndex::New, TransactionDetails};
use gloo_console::log;
use gloo_storage::{LocalStorage, Storage};
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
    constants::{
        self, STORAGE_KEY_BLINDED_UNSPENTS, STORAGE_KEY_DESCRIPTOR_ENCRYPTED,
        STORAGE_KEY_TRANSACTIONS, STORAGE_KEY_UNSPENTS,
    },
    structs::{OutPoint, ThinAsset},
};

use operations::{
    bitcoin::{create_transaction, get_mnemonic, get_wallet, save_mnemonic},
    rgb::{accept_transfer, blind_utxo, get_asset, get_assets, transfer_asset, validate_transfer},
};

pub use utils::{resolve, set_panic_hook, to_string};

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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultData {
    pub descriptor: String,
    pub change_descriptor: String,
    pub pubkey_hash: String,
}

impl SerdeEncryptSharedKey for VaultData {
    type S = BincodeSerializer<Self>; // you can specify serializer implementation (or implement it by yourself).
}

#[wasm_bindgen]
pub fn get_vault(password: String) -> Promise {
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

    let encrypted_descriptors: Result<Vec<u8>, gloo_storage::errors::StorageError> =
        LocalStorage::get(STORAGE_KEY_DESCRIPTOR_ENCRYPTED);
    let encrypted_message =
        EncryptedMessage::deserialize(encrypted_descriptors.unwrap_or_default());
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

        let (mnemonic, descriptor, change_descriptor, pubkey_hash) = get_mnemonic(&seed_password);
        let vault_data = VaultData {
            descriptor,
            change_descriptor,
            pubkey_hash,
        };
        let encrypted_message = vault_data
            .encrypt(&SharedKey::from_array(shared_key))
            .unwrap();
        let serialized_encrypted_message: Vec<u8> = encrypted_message.serialize();
        LocalStorage::set(
            STORAGE_KEY_DESCRIPTOR_ENCRYPTED,
            serialized_encrypted_message,
        )
        .unwrap_or_else(|_| {
            log!("failed at saving STORAGE_KEY_DESCRIPTOR_ENCRYPTED to local");
        });
        Ok(JsValue::from_string(mnemonic))
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

        let (descriptor, change_descriptor, pubkey_hash) =
            save_mnemonic(&seed_password, mnemonic.clone());
        let vault_data = VaultData {
            descriptor,
            change_descriptor,
            pubkey_hash,
        };
        let encrypted_message = vault_data
            .encrypt(&SharedKey::from_array(shared_key))
            .unwrap();
        let serialized_encrypted_message: Vec<u8> = encrypted_message.serialize();
        LocalStorage::set(
            STORAGE_KEY_DESCRIPTOR_ENCRYPTED,
            serialized_encrypted_message,
        )
        .unwrap_or_else(|_| {
            log!("failed at saving STORAGE_KEY_DESCRIPTOR_ENCRYPTED to local");
        });
        Ok(JsValue::from_string(mnemonic))
    })
}

#[derive(Serialize, Deserialize)]
pub struct WalletData {
    pub address: String,
    pub balance: String,
    pub transactions: Vec<TransactionDetails>,
}

#[wasm_bindgen]
pub fn get_wallet_data(descriptor: String, change_descriptor: String) -> Promise {
    set_panic_hook();
    future_to_promise(async {
        let wallet = get_wallet(descriptor, change_descriptor).await;
        let address = wallet
            .as_ref()
            .unwrap()
            .get_address(New)
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
        LocalStorage::set(STORAGE_KEY_UNSPENTS, unspent).unwrap_or_else(|_| {
            log!("failed at saving unspents to local");
        });
        let transactions = wallet
            .as_ref()
            .unwrap()
            .list_transactions(false)
            .unwrap_or_default();
        log!(format!("transactions: {transactions:#?}"));
        LocalStorage::set(STORAGE_KEY_TRANSACTIONS, &transactions).unwrap_or_else(|_| {
            log!("failed at saving unspents to local");
        });
        let wallet_data = WalletData {
            address,
            balance,
            transactions,
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
    descriptor: String,
    change_descriptor: String,
    asset: Option<String>,
    genesis: Option<String>,
) -> Promise {
    set_panic_hook();
    future_to_promise(async {
        let wallet = get_wallet(descriptor, change_descriptor).await;
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
    blinding: u64,
    utxo: OutPoint,
}

#[derive(Serialize, Deserialize, Clone)]
struct BlindingUtxo {
    conceal: String,
    blinding: String,
    utxo: OutPoint,
}

#[wasm_bindgen]
pub fn set_blinded_utxos() -> Promise {
    set_panic_hook();
    future_to_promise(async {
        let unspent: Result<Vec<String>, gloo_storage::errors::StorageError> =
            LocalStorage::get(STORAGE_KEY_UNSPENTS);
        log!(format!("blinded unspent: {unspent:#?}"));
        let unspent = unspent.unwrap();
        let mut blinded_unspents: HashMap<String, BlindingUtxo> = HashMap::new();
        let blinded_unspents_try: Result<
            HashMap<String, BlindingUtxo>,
            gloo_storage::errors::StorageError,
        > = LocalStorage::get(STORAGE_KEY_BLINDED_UNSPENTS);
        match blinded_unspents_try {
            Ok(blinded_unspents_try) => blinded_unspents = blinded_unspents_try,
            Err(_e) => (),
        }
        for utxo_string in unspent.iter() {
            match blinded_unspents.get(utxo_string) {
                Some(_blinding_utxo) => (),
                _ => {
                    let mut split = utxo_string.split(':');
                    let utxo = OutPoint {
                        txid: split.next().unwrap().to_string(),
                        vout: split.next().unwrap().to_string().parse::<u32>().unwrap(),
                    };
                    let (blind, utxo) = blind_utxo(utxo).await.unwrap();
                    let blinding_utxo = BlindingUtxo {
                        conceal: blind.conceal,
                        blinding: blind.blinding.to_string(),
                        utxo,
                    };
                    blinded_unspents.insert(utxo_string.to_string(), blinding_utxo.clone());
                    log!("insert");
                }
            };
        }
        log!("inserted");
        LocalStorage::set(STORAGE_KEY_BLINDED_UNSPENTS, &blinded_unspents).unwrap_or_else(|_| {
            log!("failed at saving blinding_utxo to local");
        });
        Ok(JsValue::from_string(
            serde_json::to_string(&blinded_unspents).unwrap(),
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
        let wallet = get_wallet(descriptor, change_descriptor).await.unwrap();
        let transaction = create_transaction(address, amount, &wallet).await;
        match transaction {
            Ok(transaction) => Ok(JsValue::from_string(transaction)),
            Err(e) => Ok(JsValue::from_string(format!("{} ", e))),
        }
    })
}

#[wasm_bindgen]
pub fn send_tokens(
    descriptor: String,
    change_descriptor: String,
    blinded_utxo: String,
    amount: u64,
    asset: String,
) -> Promise {
    set_panic_hook();
    log!("in rust");
    let asset: ThinAsset = serde_json::from_str(&asset).unwrap();
    log!(format!("asset: {asset:#?}"));
    future_to_promise(async move {
        let wallet = get_wallet(descriptor, change_descriptor).await.unwrap();
        log!("to the library");
        let consignment = transfer_asset(blinded_utxo, amount, asset, &wallet).await;
        log!("it's made");
        match consignment {
            Ok(consignment) => Ok(JsValue::from_string(consignment)),
            Err(e) => Ok(JsValue::from_string(format!("Error: {} ", e))),
        }
    })
}

#[wasm_bindgen]
pub fn send_tokens_full(
    descriptor: String,
    change_descriptor: String,
    utxo: String,
    amount: u64,
    asset: String,
) -> Promise {
    set_panic_hook();
    log!("in rust");
    let asset: ThinAsset = serde_json::from_str(&asset).unwrap();
    log!(format!("asset: {asset:#?}"));
    future_to_promise(async move {
        let wallet = get_wallet(descriptor, change_descriptor).await.unwrap();
        log!("to the library");
        let utxo = &utxo[5..];
        log!(utxo);
        //let utxo: &str = &utxo[5..];
        let mut split = utxo.split(':');
        let utxo = OutPoint {
            txid: split.next().unwrap().to_string(),
            vout: split.next().unwrap().to_string().parse::<u32>().unwrap(),
        };
        log!(format!("{utxo:#?}"));
        let (blind, utxo) = blind_utxo(utxo).await.unwrap();
        // let blinding_utxo = BlindingUtxo {
        //     conceal: blind.conceal.clone(),
        //     blinding: blind.blinding.to_string(),
        //     utxo: utxo.clone(),
        // };
        let consignment: String = transfer_asset(blind.conceal, amount, asset, &wallet)
            .await
            .unwrap_or_default();
        log!("it's made");
        log!(&consignment);
        let accept = accept_transfer(consignment.clone(), utxo, blind.blinding).await;
        log!("hola denueveo 3");
        match accept {
            Ok(_accept) => Ok(JsValue::from_string(consignment)),
            Err(e) => Err(JsValue::from_string(format!("Error: {} ", e))),
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
pub fn accept_transaction(consignment: String, txid: String, vout: u32, blinding: u64) -> Promise {
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
