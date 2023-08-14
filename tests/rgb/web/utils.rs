#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]
use bitmask_core::constants::BITMASK_ENDPOINT;
use gloo_net::http::Request;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::console;

pub async fn new_block() -> Result<JsValue, JsValue> {
    let bitmask_endpoint = BITMASK_ENDPOINT.read().await.to_string();
    let endpoint = format!("{bitmask_endpoint}/regtest/block");

    let request = Request::get(&endpoint)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .build();

    let request = match request {
        Ok(request) => request,
        Err(e) => return Err(JsValue::from(e.to_string())),
    };

    let response = request.send().await;
    match response {
        Ok(response) => {
            let status_code = response.status();
            if status_code == 200 {
                match response.text().await {
                    Ok(text) => Ok(JsValue::from(&text)),
                    Err(e) => Err(JsValue::from(e.to_string())),
                }
            } else {
                Err(JsValue::from(status_code))
            }
        }
        Err(e) => Err(JsValue::from(e.to_string())),
    }
}

pub async fn send_coins(address: &str, amount: &str) -> Result<JsValue, JsValue> {
    let bitmask_endpoint = BITMASK_ENDPOINT.read().await.to_string();
    let endpoint = format!("{bitmask_endpoint}/regtest/send/{address}/{amount}");

    let request = Request::get(&endpoint)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .build();

    let request = match request {
        Ok(request) => request,
        Err(e) => return Err(JsValue::from(e.to_string())),
    };

    let response = request.send().await;
    match response {
        Ok(response) => {
            let status_code = response.status();
            if status_code == 200 {
                match response.text().await {
                    Ok(text) => Ok(JsValue::from(&text)),
                    Err(e) => Err(JsValue::from(e.to_string())),
                }
            } else {
                Err(JsValue::from(status_code))
            }
        }
        Err(e) => Err(JsValue::from(e.to_string())),
    }
}
