use std::collections::BTreeMap;

use js_sys::Promise;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

use crate::rgb::structs::ContractAmount;
use crate::structs::{
    AcceptRequest, FullIssueRequest, FullRgbTransferRequest, ImportRequest, InvoiceRequest,
    IssueMediaRequest, IssueRequest, MediaRequest, PsbtRequest, PublishPsbtRequest, ReIssueRequest,
    RgbBidRequest, RgbOfferRequest, RgbRemoveTransferRequest, RgbSaveTransferRequest,
    RgbSwapRequest, RgbTransferRequest, SecretString, SignPsbtRequest, WatcherRequest,
};

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

pub mod constants {
    use super::*;

    #[wasm_bindgen]
    pub fn get_network() -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let result = crate::constants::get_network().await;

            Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            ))
        })
    }

    #[wasm_bindgen]
    pub fn switch_network(network_str: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::constants::switch_network(&network_str).await {
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
            let result = crate::constants::get_env(&key).await;

            Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            ))
        })
    }

    #[wasm_bindgen]
    pub fn set_env(key: String, value: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            crate::constants::set_env(&key, &value).await;

            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn sleep(ms: i32) -> js_sys::Promise {
        js_sys::Promise::new(&mut |resolve, _| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
                .unwrap();
        })
    }
}

pub mod bitcoin {
    use super::*;

    #[wasm_bindgen]
    pub fn hash_password(password: String) -> String {
        set_panic_hook();

        crate::bitcoin::hash_password(&SecretString(password))
            .0
            .to_owned()
    }

    #[wasm_bindgen]
    pub fn new_mnemonic(password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::new_mnemonic(&SecretString(password)).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn decrypt_wallet(hash: String, encrypted_descriptors: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::decrypt_wallet(
                &SecretString(hash),
                &SecretString(encrypted_descriptors),
            ) {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn upgrade_wallet(
        hash: String,
        encrypted_descriptors: String,
        seed_password: String,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::upgrade_wallet(
                &SecretString(hash),
                &SecretString(encrypted_descriptors),
                &SecretString(seed_password),
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
    pub fn new_wallet(hash: String, seed_password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::new_wallet(&SecretString(hash), &SecretString(seed_password))
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
    pub fn encrypt_wallet(mnemonic: String, hash: String, seed_password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::encrypt_wallet(
                &SecretString(mnemonic),
                &SecretString(hash),
                &SecretString(seed_password),
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
    pub fn get_wallet_data(descriptor: String, change_descriptor: Option<String>) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);
            match crate::bitcoin::get_wallet_data(
                &SecretString(descriptor),
                change_descriptor.as_ref(),
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
    pub fn sync_wallets() -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::sync_wallets().await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_new_address(descriptor: String, change_descriptor: Option<String>) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);
            match crate::bitcoin::get_new_address(
                &SecretString(descriptor),
                change_descriptor.as_ref(),
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
    pub fn send_sats(
        descriptor: String,
        change_descriptor: String,
        destination: String,
        amount: u64,
        fee_rate: Option<f32>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::send_sats(
                &SecretString(descriptor),
                &SecretString(change_descriptor),
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
        asset_address: String,
        uda_address: String,
        fee_rate: Option<f32>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::fund_vault(
                &SecretString(descriptor),
                &SecretString(change_descriptor),
                &asset_address,
                &uda_address,
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
            match crate::bitcoin::get_assets_vault(
                &SecretString(rgb_assets_descriptor_xpub),
                &SecretString(rgb_udas_descriptor_xpub),
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
    pub fn drain_wallet(
        destination: String,
        descriptor: String,
        change_descriptor: Option<String>,
        fee_rate: Option<f32>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);

            match crate::bitcoin::drain_wallet(
                &destination,
                &SecretString(descriptor),
                change_descriptor.as_ref(),
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
    pub fn bump_fee(
        txid: String,
        fee_rate: f32,
        descriptor: String,
        change_descriptor: String,
        broadcast: bool,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::bump_fee(
                txid,
                fee_rate,
                &SecretString(descriptor),
                &SecretString(change_descriptor),
                broadcast,
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
}

pub mod rgb {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen]
    pub fn issue_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: IssueRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::issue_contract(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen]
    pub fn full_issue_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let pre_req: FullIssueRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let media = match pre_req.meta {
                Some(media) => {
                    let media = crate::rgb::import_uda_data(media).await;
                    match media {
                        Ok(media) => Some(IssueMediaRequest::from(media)),
                        Err(err) => return Err(JsValue::from_string(err.to_string())),
                    }
                }
                None => None,
            };
            let req = IssueRequest {
                ticker: pre_req.ticker,
                name: pre_req.name,
                description: pre_req.description,
                supply: pre_req.supply,
                precision: pre_req.precision,
                seal: pre_req.seal,
                iface: pre_req.iface,
                meta: media,
            };
            match crate::rgb::issue_contract(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen]
    pub fn reissue_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: ReIssueRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::reissue_contract(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn rgb_create_invoice(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: InvoiceRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::create_invoice(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_psbt(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: PsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::create_psbt(&nostr_hex_sk, psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn psbt_sign_file(_nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: SignPsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::bitcoin::sign_psbt_file(psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn psbt_publish_file(_nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: PublishPsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::bitcoin::publish_psbt_file(psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn psbt_sign_and_publish_file(_nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: SignPsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::bitcoin::sign_and_publish_psbt_file(psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn transfer_asset(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let pay_req: RgbTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::transfer_asset(&nostr_hex_sk, pay_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn full_transfer_asset(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let pay_req: FullRgbTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::full_transfer_asset(&nostr_hex_sk, pay_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn accept_transfer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let pay_req: AcceptRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::accept_transfer(&nostr_hex_sk, pay_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn verify_transfers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::verify_transfers(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_contract(nostr_hex_sk: String, contract_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::get_contract(&nostr_hex_sk, &contract_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    pub fn get_simple_contract(nostr_hex_sk: String, contract_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::get_simple_contract(&nostr_hex_sk, &contract_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn hidden_contract(nostr_hex_sk: String, contract_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::hidden_contract(&nostr_hex_sk, &contract_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_contracts(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_contracts(&nostr_hex_sk, true).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_all_contracts(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_contracts(&nostr_hex_sk, false).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_interfaces(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_interfaces(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_schemas(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_schemas(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn import_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: ImportRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::import(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_watcher(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let pay_req: WatcherRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::create_watcher(&nostr_hex_sk, pay_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_details(nostr_hex_sk: String, name: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::watcher_details(&nostr_hex_sk, &name).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn clear_watcher(nostr_hex_sk: String, name: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::clear_watcher(&nostr_hex_sk, &name).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_address(nostr_hex_sk: String, name: String, address: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::watcher_address(&nostr_hex_sk, &name, &address).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_utxo(nostr_hex_sk: String, name: String, utxo: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::watcher_utxo(&nostr_hex_sk, &name, &utxo).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_next_address(nostr_hex_sk: String, name: String, iface: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::watcher_next_address(&nostr_hex_sk, &name, &iface).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_next_utxo(nostr_hex_sk: String, name: String, iface: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::watcher_next_utxo(&nostr_hex_sk, &name, &iface).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_unspent_utxos(nostr_hex_sk: String, name: String, iface: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::watcher_unspent_utxos(&nostr_hex_sk, &name, &iface).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_transfers(nostr_hex_sk: String, contract_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_transfers(&nostr_hex_sk, contract_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn save_transfer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbSaveTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::save_transfer(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn remove_transfer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbRemoveTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::remove_transfer(&nostr_hex_sk, req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }
    #[wasm_bindgen]
    pub fn decode_invoice(invoice: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::decode_invoice(invoice).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_offer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let offer_req: RgbOfferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::create_seller_offer(&nostr_hex_sk, offer_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_bid(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let bid_req: RgbBidRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::create_buyer_bid(&nostr_hex_sk, bid_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_swap(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let swap_req: RgbSwapRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::create_swap_transfer(&nostr_hex_sk, swap_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn direct_swap(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let bid_req: RgbBidRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::direct_swap_transfer(&nostr_hex_sk, bid_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn public_offers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_public_offers(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn my_orders(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_my_orders(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn my_offers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_my_offers(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn my_bids(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_my_bids(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn import_consignments(request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: BTreeMap<String, String> = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::import_consignments(req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_consignment(consig_or_receipt_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::get_consignment(&consig_or_receipt_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_media_metadata(media_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::get_media_metadata(&media_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn import_uda_data(request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: MediaRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::import_uda_data(req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn contract_amount(amount: u64, precision: u8) -> u64 {
        set_panic_hook();
        ContractAmount::new(amount, precision).to_value()
    }

    #[wasm_bindgen]
    pub fn contract_amount_str(amount: u64, precision: u8) -> String {
        set_panic_hook();
        ContractAmount::new(amount, precision).to_string()
    }

    #[wasm_bindgen]
    pub fn contract_amount_parse_str(amount: String, precision: u8) -> String {
        set_panic_hook();
        ContractAmount::from(amount, precision).to_string()
    }

    #[wasm_bindgen]
    pub fn contract_amount_parse_value(amount: String, precision: u8) -> u64 {
        set_panic_hook();
        ContractAmount::from(amount, precision).to_value()
    }

    #[wasm_bindgen]
    pub fn contract_amount_parse_decimal_value(amount: String) -> u64 {
        set_panic_hook();
        ContractAmount::from_decimal_str(amount).to_value()
    }
}

pub mod lightning {
    use super::*;

    #[wasm_bindgen]
    pub fn create_wallet(username: String, password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::create_wallet(&username, &password).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn auth(username: String, password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::auth(&username, &password).await {
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
            match crate::lightning::create_invoice(&description, amount, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_balance(token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::get_balance(&token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_txs(token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::get_txs(&token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn pay_invoice(payment_request: String, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::pay_invoice(&payment_request, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn check_payment(payment_hash: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::check_payment(&payment_hash).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn swap_btc_ln(token: String, ln_address: Option<String>) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::swap_btc_ln(&token, ln_address).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn swap_ln_btc(address: String, amount: u64, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::swap_ln_btc(&address, amount, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }
}

pub mod carbonado {
    use super::*;

    #[wasm_bindgen]
    pub fn store(
        secret_key: String,
        name: String,
        data: Vec<u8>,
        force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::carbonado::store(&secret_key, &name, &data, force, metadata).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn retrieve(secret_key: String, name: String) -> Promise {
        set_panic_hook();

        use js_sys::Uint8Array;

        future_to_promise(async move {
            match crate::carbonado::retrieve(&secret_key, &name, vec![]).await {
                Ok((result, _)) => {
                    let array = Uint8Array::new_with_length(result.len() as u32);
                    array.copy_from(&result);
                    Ok(JsValue::from(array))
                }
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn retrieve_metadata(secret_key: String, name: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::carbonado::retrieve_metadata(&secret_key, &name).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn encode_hex(bytes: Vec<u8>) -> String {
        set_panic_hook();

        crate::carbonado::util::encode_hex(&bytes)
    }

    #[wasm_bindgen]
    pub fn encode_base64(bytes: Vec<u8>) -> String {
        set_panic_hook();

        crate::carbonado::util::encode_base64(&bytes)
    }

    #[wasm_bindgen]
    pub fn decode_hex(string: String) -> Result<Vec<u8>, JsError> {
        set_panic_hook();

        crate::carbonado::util::decode_hex(&string).map_err(|err| JsError::new(&err.to_string()))
    }

    #[wasm_bindgen]
    pub fn decode_base64(string: String) -> Result<Vec<u8>, JsError> {
        set_panic_hook();

        crate::carbonado::util::decode_base64(&string).map_err(|err| JsError::new(&err.to_string()))
    }
}

pub mod nostr {
    use super::*;

    #[wasm_bindgen]
    pub fn new_nostr_pubkey(pubkey: String, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::nostr::new_nostr_pubkey(&pubkey, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn update_nostr_pubkey(pubkey: String, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::nostr::update_nostr_pubkey(&pubkey, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }
}
