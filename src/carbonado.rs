use amplify::hex::ToHex;
use bitcoin_30::secp256k1::{PublicKey, SecretKey};

use crate::{carbonado::error::CarbonadoError, constants::NETWORK, info, structs::FileMetadata};

pub mod error;

#[cfg(not(target_arch = "wasm32"))]
pub mod metrics;

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    auctions_retrieve, auctions_store, handle_file, marketplace_retrieve, marketplace_store,
    retrieve, retrieve_metadata, store,
};

#[cfg(not(target_arch = "wasm32"))]
mod server {
    use crate::constants::{get_coordinator_nostr_key, get_marketplace_nostr_key};

    use super::*;

    use std::{
        io::{Error, ErrorKind},
        path::PathBuf,
        str::FromStr,
    };

    use bitcoin_30::secp256k1::ecdh::SharedSecret;
    use tokio::fs;

    pub async fn store(
        sk: &str,
        name: &str,
        input: &[u8],
        _force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Result<(), CarbonadoError> {
        let level = 15;
        let sk = hex::decode(sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
        let filepath = handle_file(&pk_hex, name, body.len()).await?;
        fs::write(&filepath, body).await?;
        // metrics::update(&filepath).await?;
        Ok(())
    }

    pub async fn marketplace_store(
        name: &str,
        input: &[u8],
        metadata: Option<Vec<u8>>,
    ) -> Result<(PathBuf, Vec<u8>), CarbonadoError> {
        let marketplace_key: String = get_marketplace_nostr_key().await;

        let level = 15;
        let sk = hex::decode(marketplace_key)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
        let filepath = handle_file(&pk_hex, name, body.len()).await?;
        fs::write(&filepath, body.clone()).await?;
        // metrics::update(&filepath).await?;
        Ok((filepath, body))
    }

    pub async fn auctions_store(
        bundle_id: &str,
        name: &str,
        input: &[u8],
        metadata: Option<Vec<u8>>,
    ) -> Result<(PathBuf, Vec<u8>), CarbonadoError> {
        let coordinator_key: String = get_coordinator_nostr_key().await;

        let level = 15;
        let coordinator_sk = hex::decode(coordinator_key)?;
        let coordinator_secret_key = SecretKey::from_slice(&coordinator_sk)?;
        let bundle_public_key =
            PublicKey::from_str(bundle_id).map_err(|_| CarbonadoError::WrongNostrPublicKey)?;

        let share_sk = SharedSecret::new(&bundle_public_key, &coordinator_secret_key);
        let share_sk = share_sk.display_secret().to_string();

        let sk = hex::decode(share_sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);

        let pk = public_key.serialize();
        let pk_hex = public_key.to_hex();

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
        let filepath = handle_file(&pk_hex, name, body.len()).await?;
        fs::write(filepath.clone(), body.clone()).await?;
        Ok((filepath, body))
    }

    pub async fn retrieve(
        sk: &str,
        name: &str,
        alt_names: Vec<&String>,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        use crate::rgb::constants::RGB_STRICT_TYPE_VERSION;

        let sk = hex::decode(sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let mut final_name = name.to_string();
        let network = NETWORK.read().await.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        if let Ok(bytes) = fs::read(filepath).await {
            let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        // Check alternative names
        let alt_names = alt_names.into_iter().map(|x| format!("{network}-{x}"));
        for alt_name in alt_names {
            let filepath = handle_file(&pk, &alt_name, 0).await?;
            if let Ok(bytes) = fs::read(filepath).await {
                let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
                if let Some(metadata) = header.metadata {
                    if metadata == RGB_STRICT_TYPE_VERSION {
                        return Ok((decoded, header.metadata.map(|m| m.to_vec())));
                    }
                }
            }
        }

        Ok((Vec::new(), None))
    }

    pub async fn marketplace_retrieve(
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        let marketplace_key: String = get_marketplace_nostr_key().await;

        let sk = hex::decode(marketplace_key)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let mut final_name = name.to_string();
        let network = NETWORK.read().await.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        if let Ok(bytes) = fs::read(filepath).await {
            let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        Ok((Vec::new(), None))
    }

    pub async fn auctions_retrieve(
        bundle_id: &str,
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        let coordinator_key: String = get_coordinator_nostr_key().await;

        let coordinator_sk = hex::decode(coordinator_key)?;
        let coordinator_secret_key = SecretKey::from_slice(&coordinator_sk)?;
        let bundle_public_key =
            PublicKey::from_str(bundle_id).map_err(|_| CarbonadoError::WrongNostrPublicKey)?;

        let share_sk = SharedSecret::new(&bundle_public_key, &coordinator_secret_key);
        let share_sk = share_sk.display_secret().to_string();

        let sk = hex::decode(share_sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);

        let pk = public_key.to_hex();

        let mut final_name = name.to_string();
        let network = NETWORK.read().await.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        if let Ok(bytes) = fs::read(filepath).await {
            let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        Ok((Vec::new(), None))
    }

    pub async fn handle_file(
        pk: &str,
        name: &str,
        bytes: usize,
    ) -> Result<PathBuf, CarbonadoError> {
        let mut final_name = name.to_string();
        let network = NETWORK.read().await.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let directory = std::path::Path::new(
            &std::env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned()),
        )
        .join(pk);

        let filepath = directory.join(final_name);
        let filedir = filepath.parent().unwrap();
        fs::create_dir_all(filedir).await.map_err(|_| {
            CarbonadoError::StdIoError(Error::new(
                ErrorKind::NotFound,
                format!("Cannot create filepath to carbonado file {name}"),
            ))
        })?;
        if bytes == 0 {
            info!(format!("read {}", filepath.to_string_lossy()));
        } else {
            info!(format!(
                "write {bytes} bytes to {}",
                filepath.to_string_lossy()
            ));
        }

        Ok(filepath)
    }

    pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
        let sk = hex::decode(sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let network = NETWORK.read().await.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];

        let mut final_name = name.to_string();
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        let bytes = fs::read(filepath).await?;

        let (header, _) = carbonado::file::decode(&sk, &bytes)?;

        let result = FileMetadata {
            filename: header.file_name(),
            metadata: header.metadata.unwrap_or_default(),
        };

        Ok(result)
    }
}

#[cfg(target_arch = "wasm32")]
pub use client::{
    auctions_retrieve, auctions_store, marketplace_retrieve, marketplace_store, retrieve,
    retrieve_metadata, store,
};

#[cfg(target_arch = "wasm32")]
mod client {
    use super::*;
    use js_sys::{Array, Promise, Uint8Array};
    use serde::Deserialize;
    use std::sync::Arc;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::{future_to_promise, JsFuture};

    use gloo_net::http::Request;
    use gloo_utils::errors::JsError;

    use crate::constants::CARBONADO_ENDPOINT;

    fn js_to_error(js_value: JsValue) -> CarbonadoError {
        CarbonadoError::JsError(js_to_js_error(js_value))
    }

    fn js_to_js_error(js_value: JsValue) -> JsError {
        match JsError::try_from(js_value) {
            Ok(error) => error,
            Err(_) => unreachable!("JsValue passed is not an Error type -- this is a bug"),
        }
    }

    #[derive(Debug, Deserialize)]
    struct PostStorePromiseResult {
        value: f64,
    }

    pub async fn store(
        sk: &str,
        name: &str,
        input: &[u8],
        force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Result<(), CarbonadoError> {
        let level = 15;
        let sk = hex::decode(sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
        let body = Arc::new(body);
        let network = NETWORK.read().await.to_string();

        let mut force_write = "";
        if force {
            force_write = "/force";
        }

        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();
        let requests = Array::new();
        for endpoint in endpoints {
            let url = format!("{endpoint}/{pk_hex}/{network}-{name}{force_write}");
            let fetch_fn = future_to_promise(fetch_post(url, body.clone())); // TODO: try using .value_of();
            requests.push(&fetch_fn);
        }

        let results = JsFuture::from(Promise::all_settled(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Store results: {results:?}"));

        let results = serde_wasm_bindgen::from_value::<Vec<PostStorePromiseResult>>(results)?;
        let success = results.iter().any(|result| result.value == 200.0);
        if success {
            Ok(())
        } else {
            Err(CarbonadoError::AllEndpointsFailed)
        }
    }

    pub async fn marketplace_store(
        name: &str,
        input: &[u8],
        _metadata: Option<Vec<u8>>,
    ) -> Result<(), CarbonadoError> {
        let body = Arc::new(input.to_vec());
        let network = NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();
        let requests = Array::new();

        for endpoint in endpoints {
            let url = format!("{endpoint}/server/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_post(url, body.clone()));
            requests.push(&fetch_fn);
        }

        let results = JsFuture::from(Promise::all_settled(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Store results: {results:?}"));

        let results = serde_wasm_bindgen::from_value::<Vec<PostStorePromiseResult>>(results)?;
        let success = results.iter().any(|result| result.value == 200.0);
        if success {
            Ok(())
        } else {
            Err(CarbonadoError::AllEndpointsFailed)
        }
    }

    pub async fn auctions_store(
        _bundle_id: &str,
        _name: &str,
        _input: &[u8],
        _metadata: Option<Vec<u8>>,
    ) -> Result<(), CarbonadoError> {
        todo!()
    }

    pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
        let sk = hex::decode(sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let network = NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints {
            let url = format!("{endpoint}/{pk}/{network}-{name}/metadata");
            let fetch_fn = future_to_promise(fetch_get_text(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Retrieve metadata result: {result:?}"));

        let result: FileMetadata = serde_json::from_str(&result.as_string().unwrap())?;
        Ok(result)
    }

    pub async fn retrieve(
        sk: &str,
        name: &str,
        alt_names: Vec<&String>,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        use carbonado::file::Header;

        let sk = hex::decode(sk)?;
        let secret_key = SecretKey::from_slice(&sk)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let network = NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints.iter() {
            let url = format!("{endpoint}/{pk}/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_get_byte_array(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        let array = Uint8Array::from(result);
        let encoded = array.to_vec();

        if encoded.len() > Header::len() {
            let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        // Check alternative names
        for alt_name in alt_names {
            let requests = Array::new();
            for endpoint in endpoints.iter() {
                let url = format!("{endpoint}/{pk}/{network}-{alt_name}");

                let fetch_fn = future_to_promise(fetch_get_byte_array(url));
                requests.push(&fetch_fn);
            }

            let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
                .await
                .map_err(js_to_error)?;

            let array = Uint8Array::from(result);
            let encoded = array.to_vec();

            if encoded.len() > Header::len() {
                let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
                return Ok((decoded, header.metadata.map(|m| m.to_vec())));
            }
        }

        Ok((Vec::new(), None))
    }

    pub async fn marketplace_retrieve(
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        let network = NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints.iter() {
            let url = format!("{endpoint}/server/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_get_byte_array(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        let array = Uint8Array::from(result);
        let encoded = array.to_vec();

        Ok((encoded.to_vec(), None))
    }

    pub async fn auctions_retrieve(
        _bundle_id: &str,
        _name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        todo!()
    }

    async fn fetch_post(url: String, body: Arc<Vec<u8>>) -> Result<JsValue, JsValue> {
        let array = Uint8Array::new_with_length(body.len() as u32);
        array.copy_from(&body);

        let request = Request::post(&url)
            .header("Content-Type", "application/octet-stream")
            .header("Cache-Control", "no-cache")
            .body(array);

        let request = match request {
            Ok(request) => request,
            Err(e) => return Err(JsValue::from(e.to_string())),
        };

        let response = request.send().await;

        match response {
            Ok(response) => {
                let status_code = response.status();
                if status_code == 200 {
                    Ok(JsValue::from(status_code))
                } else {
                    Err(JsValue::from(status_code))
                }
            }
            Err(e) => Err(JsValue::from(e.to_string())),
        }
    }

    async fn fetch_get_text(url: String) -> Result<JsValue, JsValue> {
        let request = Request::get(&url)
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

    async fn fetch_get_byte_array(url: String) -> Result<JsValue, JsValue> {
        let request = Request::get(&url)
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
                    match response.binary().await {
                        Ok(bytes) => {
                            let array = Uint8Array::new_with_length(bytes.len() as u32);
                            array.copy_from(&bytes);
                            Ok(JsValue::from(&array))
                        }
                        Err(e) => Err(JsValue::from(e.to_string())),
                    }
                } else {
                    Err(JsValue::from(status_code))
                }
            }
            Err(e) => Err(JsValue::from(e.to_string())),
        }
    }
}

// Utility functions for handling data of different encodings
pub mod util {
    use super::*;

    pub fn encode_hex(bytes: &[u8]) -> String {
        hex::encode(bytes)
    }

    pub fn encode_base64(bytes: &[u8]) -> String {
        base64::encode(bytes)
    }

    pub fn decode_hex(string: &str) -> Result<Vec<u8>, CarbonadoError> {
        Ok(hex::decode(string)?)
    }

    pub fn decode_base64(string: &str) -> Result<Vec<u8>, CarbonadoError> {
        Ok(base64::decode(string)?)
    }
}
