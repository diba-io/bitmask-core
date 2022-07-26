use anyhow::{Context, Result};
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
pub async fn post_json<T: Serialize>(url: String, body: &T) -> Result<(String, u16)> {
    let response = Request::post(&url)
        .body(serde_json::to_string(body)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await
        .context(format!("Error sending JSON POST request to {}", url))?;

    let status_code = response.status();

    let response_text = response
        .text()
        .await
        .context("Error in handling server response")?;

    Ok((response_text, status_code))
}

#[cfg(target_arch = "wasm32")]
pub async fn get(url: String) -> Result<(String, u16)> {
    let response = Request::get(&url)
        .send()
        .await
        .context(format!("Error sending GET request to {}", url))?;

    let status_code = response.status();

    let response_text = response
        .text()
        .await
        .context("Error in handling server response")?;

    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_json<T: Serialize>(url: String, body: &T) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .body(serde_json::to_string(body)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await
        .context(format!("Error sending JSON POST request to {}", url))?;

    let status_code = response.status().as_u16();

    let response_text = response
        .text()
        .await
        .context("Error in handling server response")?;

    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get(url: String) -> Result<(String, u16)> {
    let response = reqwest::get(&url)
        .await
        .context(format!("Error sending GET request to {}", url))?;

    let status_code = response.status().as_u16();

    let response_text = response
        .text()
        .await
        .context("Error in handling server response")?;

    Ok((response_text, status_code))
}
