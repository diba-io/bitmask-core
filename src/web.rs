#![allow(unused_variables)]
use js_sys::Promise;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
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
pub fn get_encrypted_wallet(password: String, encrypted_descriptors: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_encrypted_wallet(&password, &encrypted_descriptors) {
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

// #[wasm_bindgen]
// pub fn list_assets(xpubkh: String, encryption_secret: String) -> Promise {
//     set_panic_hook();

//     future_to_promise(async move {
//         // TODO: make call to storage lambda, decrypt, then pass to list_assets
//         match crate::list_assets(&asset, &rgb_descriptor_xpub).await {
//             Ok(result) => Ok(JsValue::from_string(
//                 serde_json::to_string(&result).unwrap(),
//             )),
//             Err(err) => Err(JsValue::from_string(err.to_string())),
//         }
//     })
// }

#[wasm_bindgen]
pub fn import_asset(asset: String, rgb_descriptor_xpub: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::import_asset(&asset, &rgb_descriptor_xpub).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn get_blinded_utxo(utxo_string: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_blinded_utxo(&utxo_string).await {
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
    fee_rate: Option<f32>,
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
pub fn fund_vault(
    descriptor: String,
    change_descriptor: String,
    address: String,
    uda_address: String,
    asset_amount: u64,
    uda_amount: u64,
    fee_rate: Option<f32>,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::fund_vault(
            &descriptor,
            &change_descriptor,
            &address,
            &uda_address,
            asset_amount,
            uda_amount,
            fee_rate,
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
pub fn get_assets_vault(
    rgb_assets_descriptor_xpub: String,
    rgb_udas_descriptor_xpub: String,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_assets_vault(&rgb_assets_descriptor_xpub, &rgb_udas_descriptor_xpub).await
        {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn create_asset(
    ticker: String,
    name: String,
    precision: u8,
    supply: u64,
    utxo: String,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::create_asset(&ticker, &name, precision, supply, &utxo).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn send_assets(
    btc_descriptor_xprv: String,
    btc_change_descriptor_xprv: String,
    rgb_assets_descriptor_xprv: String,
    rgb_assets_descriptor_xpub: String,
    blinded_utxo: String,
    amount: u64,
    asset_contract: String,
    fee_rate: f32,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::send_assets(
            &btc_descriptor_xprv,
            &btc_change_descriptor_xprv,
            &rgb_assets_descriptor_xprv,
            &rgb_assets_descriptor_xpub,
            &blinded_utxo,
            amount,
            &asset_contract,
            fee_rate,
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
pub fn accept_transfer(consignment: String, reveal: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::accept_transfer(&consignment, &reveal).await {
            Ok(result) => {
                if result.valid {
                    Ok(JsValue::from_string(
                        serde_json::to_string(&result).unwrap(),
                    ))
                } else {
                    Err(JsValue::from_string(format!(
                        "invalid due to erroneous endpoints with id {}",
                        result.id
                    )))
                }
            }
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn get_network() -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::get_network() {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn switch_network(network_str: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::switch_network(&network_str).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn get_endpoint(path: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let result = crate::get_endpoint(&path).await;
        Ok(JsValue::from_string(
            serde_json::to_string(&result).unwrap(),
        ))
    })
}

#[wasm_bindgen]
pub fn switch_host(host: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        crate::switch_host(&host).await;
        Ok(JsValue::UNDEFINED)
    })
}
