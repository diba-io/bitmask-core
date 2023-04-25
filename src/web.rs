#![allow(unused_variables)]
use crate::data::structs::{AcceptRequest, PsbtRequest, RgbTransferRequest};
use crate::lightning;
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
        match crate::new_mnemonic_seed(&encryption_password, &seed_password) {
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
// pub fn import_asset(asset: String, utxo: String) -> Promise {
//     set_panic_hook();

//     future_to_promise(async move {
//         match crate::import_asset(&asset, &utxo).await {
//             Ok(result) => Ok(JsValue::from_string(
//                 serde_json::to_string(&result).unwrap(),
//             )),
//             Err(err) => Err(JsValue::from_string(err.to_string())),
//         }
//     })
// }

#[wasm_bindgen]
pub fn send_sats(
    descriptor: String,
    change_descriptor: String,
    destination: String,
    amount: u64,
    fee_rate: Option<f32>,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::send_sats(
            &descriptor,
            &change_descriptor,
            &destination,
            amount,
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
pub fn issue_contract(
    ticker: String,
    name: String,
    description: String,
    precision: u8,
    supply: u64,
    seal: String,
    iface: String,
) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::issue_contract(
            &ticker,
            &name,
            &description,
            precision,
            supply,
            &seal,
            &iface,
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
pub fn create_invoice(contract_id: String, iface: String, amount: u64, seal: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::create_invoice(&contract_id, &iface, amount, &seal).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn create_psbt(request: JsValue) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let psbt_req: PsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
        match crate::create_psbt(psbt_req).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn pay_asset(request: JsValue) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let pay_req: RgbTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
        match crate::pay_asset(pay_req).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn accept_transfer(request: JsValue) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let pay_req: AcceptRequest = serde_wasm_bindgen::from_value(request).unwrap();
        match crate::accept_transfer(pay_req).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn list_contracts() -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::list_contracts().await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn list_interfaces() -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::list_interfaces().await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn list_schemas() -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match crate::list_schemas().await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

// #[wasm_bindgen]
// pub fn sign_psbt(rgb_descriptor_xprv: String, psbt: String) -> Promise {
//     set_panic_hook();

//     future_to_promise(async move {
//         match crate::sign_psbt_web(&rgb_descriptor_xprv, &psbt).await {
//             Ok(result) => Ok(JsValue::from_string(
//                 serde_json::to_string(&result).unwrap(),
//             )),
//             Err(err) => Err(JsValue::from_string(err.to_string())),
//         }
//     })
// }

#[wasm_bindgen]
pub fn get_network() -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let result = crate::get_network();

        Ok(JsValue::from_string(
            serde_json::to_string(&result).unwrap(),
        ))
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
pub fn get_env(key: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        let result = crate::get_env(&key).await;

        Ok(JsValue::from_string(
            serde_json::to_string(&result).unwrap(),
        ))
    })
}

#[wasm_bindgen]
pub fn set_env(key: String, value: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        crate::set_env(&key, &value).await;

        Ok(JsValue::UNDEFINED)
    })
}

#[wasm_bindgen]
pub fn ln_create_wallet(username: String, password: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::create_wallet(&username, &password).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn ln_auth(username: String, password: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::auth(&username, &password).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn ln_create_invoice(description: String, amount: u32, token: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::create_invoice(&description, amount, &token).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn ln_get_balance(token: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::get_balance(&token).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn ln_get_txs(token: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::get_txs(&token).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn ln_pay_invoice(payment_request: String, token: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::pay_invoice(&payment_request, &token).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}

#[wasm_bindgen]
pub fn ln_check_payment(payment_hash: String) -> Promise {
    set_panic_hook();

    future_to_promise(async move {
        match lightning::check_payment(&payment_hash).await {
            Ok(result) => Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            )),
            Err(err) => Err(JsValue::from_string(err.to_string())),
        }
    })
}
