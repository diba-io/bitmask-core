use js_sys::Promise;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use crate::data::structs::ThinAsset;

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
        match crate::get_vault(password, encrypted_descriptors) {
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
        match crate::get_mnemonic_seed(encryption_password, seed_password) {
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
        match crate::save_mnemonic_seed(mnemonic, encryption_password, seed_password) {
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
        match crate::get_wallet_data(descriptor, change_descriptor).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn import_list_assets(
    xpubkh: String,
    encryption_secret: String,
    node_url: Option<String>,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        todo!("make call to storage lambda, decrypt, then pass to import_list_assets");
        let result = get(url(node_url)).await;

        match crate::import_list_assets(node_url).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn import_asset(
    rgb_tokens_descriptor: String,
    asset: Option<String>,
    genesis: Option<String>,
    node_url: Option<String>,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::import_asset(rgb_tokens_descriptor, asset, genesis, node_url).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn set_blinded_utxo(utxo_string: String, node_url: Option<String>) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::set_blinded_utxo(utxo_string, node_url).await {
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
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::send_sats(descriptor, change_descriptor, address, amount).await {
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
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::fund_wallet(descriptor, change_descriptor, address, uda_address).await {
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
    node_url: Option<String>,
) -> Promise {
    set_panic_hook();

    let asset: ThinAsset = serde_json::from_str(&asset).unwrap();

    future_to_promise(async move {
        match crate::send_tokens(
            btc_descriptor,
            btc_change_descriptor,
            rgb_tokens_descriptor,
            blinded_utxo,
            amount,
            asset,
            node_url,
        )
        .await
        {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn validate_transaction(utxo_string: String, node_url: Option<String>) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::validate_transaction(utxo_string, node_url).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn accept_transaction(
    consignment: String,
    txid: String,
    vout: u32,
    blinding: String,
    node_url: Option<String>,
) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
        match crate::accept_transaction(consignment, txid, vout, blinding, node_url).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn import_accept(
    rgb_tokens_descriptor: String,
    asset: String,
    consignment: String,
    txid: String,
    vout: u32,
    blinding: String,
    node_url: Option<String>,
) -> Promise {
    set_panic_hook();
    future_to_promise(async move {
        match crate::import_accept(
            rgb_tokens_descriptor,
            asset,
            consignment,
            txid,
            vout,
            blinding,
            node_url,
        )
        .await
        {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}
