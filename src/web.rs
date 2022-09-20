#![allow(unused_variables)]
use js_sys::Promise;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use bdk::FeeRate;

use crate::{data::structs::ThinAsset, constants::{
    BLINDED_UTXO_ENDPOINT, LIST_ASSETS_ENDPOINT, IMPORT_ASSET_ENDPOINT, SEND_ASSETS_ENDPOINT, ACCEPT_TRANSFER_ENDPOINT, VALIDATE_TRANSFER_ENDPOINT}, util::get};

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

pub async fn resolve(promise: Promise) -> JsValue {
    JsFuture::from(promise).await.unwrap()
}

pub fn to_string(js_str: &JsValue) -> String {
    js_str.as_string().unwrap()
}

pub fn json_parse<T: DeserializeOwned>(js_str: &JsValue) -> T {
    serde_json::from_str(&js_str.as_string().unwrap()).expect("parsed json")
}

trait FromString {
    fn from_string(str: String) -> JsValue;
}

impl FromString for JsValue {
    fn from_string(str: String) -> JsValue {
        JsValue::from_str(&str)
    }
}

#[wasm_bindgen]
pub fn get_vault(password: String, encrypted_descriptors: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_vault(&password, &encrypted_descriptors) {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn get_mnemonic_seed(encryption_password: String, seed_password: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_mnemonic_seed(&encryption_password, &seed_password) {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
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
        match crate::save_mnemonic_seed(&mnemonic, &encryption_password, &seed_password) {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn get_wallet_data(descriptor: String, change_descriptor: Option<String>) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_wallet_data(&descriptor, change_descriptor).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn list_assets(xpubkh: String, encryption_secret: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        // TODO: make call to storage lambda, decrypt, then pass to list_assets
        let result = get(&LIST_ASSETS_ENDPOINT).await;
        match result {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn import_asset(
    genesis: String,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let result = get(&IMPORT_ASSET_ENDPOINT).await;
        match result {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn set_blinded_utxo(utxo_string: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let result = get(&BLINDED_UTXO_ENDPOINT).await;
        match result {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn send_sats(
    descriptor: String,
    change_descriptor: String,
    address: String,
    amount: u64,
    fee_rate: Option<FeeRate>,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::send_sats(&descriptor, &change_descriptor, &address, amount, fee_rate).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn fund_wallet(
    descriptor: String,
    change_descriptor: String,
    address: String,
    uda_address: String,
    fee_rate: Option<FeeRate>,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::fund_wallet(&descriptor, &change_descriptor, &address, &uda_address, fee_rate).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn send_tokens(
    btc_descriptor: String,
    btc_change_descriptor: String,
    rgb_tokens_descriptor: String,
    blinded_utxo: String,
    amount: u64,
    asset: String,
) -> Promise {
    set_panic_hook();

    let asset: ThinAsset = serde_json::from_str(&asset).unwrap();

    future_to_promise(async move {
        let result = get(&SEND_ASSETS_ENDPOINT).await;
        match result
        {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn validate_transfer(utxo_string: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let result = get(&VALIDATE_TRANSFER_ENDPOINT).await;
        match result{
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn accept_transfer(
    consignment: String,
    txid: String,
    vout: u32,
    blinding: String,
) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
        let result = get(&ACCEPT_TRANSFER_ENDPOINT).await;
        match result
        {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}
