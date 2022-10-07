use anyhow::{Context, Result};
use bech32::{decode, encode, FromBase32, ToBase32, Variant};
#[cfg(target_arch = "wasm32")]
use gloo_net::http::Request;
use serde::Serialize;

#[macro_export]
macro_rules! info {
    ($($arg:expr),+) => {
        let output = vec![$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::info!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::info!("{}", output);
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:expr),+) => {
        let output = vec![$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::debug!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::debug!("{}", output);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:expr),+) => {
        let output = vec![$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::error!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::error!("{}", output);
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:expr),+) => {
        let output = vec![$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::warn!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::warn!("{}", output);
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:expr),+) => {
        let output = vec![$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::trace!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::trace!("{}", output);
    };
}

#[cfg(target_arch = "wasm32")]
pub async fn post_json<T: Serialize>(url: &str, body: &T) -> Result<(String, u16)> {
    let response = Request::post(url)
        .body(serde_json::to_string(body)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    Ok((response_text, status_code))
}

#[cfg(target_arch = "wasm32")]
pub async fn get(url: &str) -> Result<(String, u16)> {
    let response = Request::get(url)
        .send()
        .await
        .context(format!("Error sending GET request to {url}"))?;

    let status_code = response.status();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for GET request to {url}"
    ))?;

    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_json<T: Serialize>(url: &str, body: &Option<T>) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let mut response = client.post(url);
    if let Some(b) = body {
        response = response.json(&b);
    }
    let response = response
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get(url: &str, token: &str) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context(format!("Error sending GET request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for GET request to {url}"
    ))?;

    Ok((response_text, status_code))
}

pub fn bech32_encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    Ok(encode(hrp, bytes.to_base32(), Variant::Bech32)?)
}

pub fn bech32m_encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    Ok(encode(hrp, bytes.to_base32(), Variant::Bech32m)?)
}

pub fn bech32_decode(bech32_str: &str) -> Result<(String, Vec<u8>, Variant)> {
    let (hrp, words, variant) = decode(bech32_str)?;
    Ok((hrp, Vec::<u8>::from_base32(&words)?, variant))
}
