use amplify::hex::ToHex;
use bitcoin_30::secp256k1::{PublicKey, SecretKey};
#[cfg(not(feature = "server"))]
use gloo_utils::errors::JsError;
#[cfg(not(feature = "server"))]
use js_sys::{Promise, Uint8Array};
#[cfg(feature = "server")]
use std::path::PathBuf;
use std::{
    io::{Error, ErrorKind},
    sync::Arc,
};
#[cfg(feature = "server")]
use tokio::fs;
#[cfg(not(feature = "server"))]
use wasm_bindgen::JsValue;

pub mod error;

use crate::{carbonado::error::CarbonadoError, constants::NETWORK, info, structs::FileMetadata};

#[cfg(not(feature = "server"))]
use crate::constants::CARBONADO_ENDPOINT;

#[cfg(not(feature = "server"))]
fn js_to_error(js_value: JsValue) -> CarbonadoError {
    CarbonadoError::JsError(js_to_js_error(js_value))
}

#[cfg(not(feature = "server"))]
fn js_to_js_error(js_value: JsValue) -> JsError {
    match JsError::try_from(js_value) {
        Ok(error) => error,
        Err(_) => unreachable!("JsValue passed is not an Error type -- this is a bug"),
    }
}

#[cfg(not(feature = "server"))]
async fn store_fetch(url: &str, body: Arc<Vec<u8>>) -> Promise {
    let array = Uint8Array::new_with_length(body.len() as u32);
    array.copy_from(&body);

    let response = gloo_net::http::Request::post(url)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .body(array)
        .unwrap()
        .send()
        .await;

    match response {
        Ok(response) => {
            let status_code = response.status();
            if status_code == 200 {
                Promise::resolve(&JsValue::from(status_code))
            } else {
                Promise::reject(&JsValue::from(status_code))
            }
        }
        Err(e) => Promise::reject(&JsValue::from(e.to_string())),
    }
}

#[cfg(not(feature = "server"))]
pub async fn store(
    sk: &str,
    name: &str,
    input: &[u8],
    force: bool,
    metadata: Option<Vec<u8>>,
) -> Result<(), CarbonadoError> {
    use js_sys::Array;
    use wasm_bindgen_futures::JsFuture;

    let level = 15;
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.serialize();
    let pk_hex = hex::encode(pk);

    let meta: Option<[u8; 8]> = metadata.map(|m| m.try_into().expect("invalid metadata size"));
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
        let fetch_fn = JsValue::from(store_fetch(&url, body.clone()).await);
        requests.push(&fetch_fn);
    }

    let results = JsFuture::from(Promise::all_settled(&JsValue::from(requests)))
        .await
        .map_err(js_to_error)?;

    info!(format!("Store results: {results:?}"));

    let success = Array::from(&results)
        .iter()
        .any(|status| status.as_f64() == Some(200.0));

    if success {
        Ok(())
    } else {
        Err(CarbonadoError::AllEndpointsFailed)
    }
}

#[cfg(feature = "server")]
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

    let meta: Option<[u8; 8]> = metadata.map(|m| m.try_into().expect("invalid metadata size"));
    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
    let filepath = handle_file(&pk_hex, name, body.len()).await?;
    fs::write(filepath, body).await?;
    Ok(())
}

#[cfg(not(feature = "server"))]
pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let network = NETWORK.read().await.to_string();
    let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
    let endpoints: Vec<&str> = endpoints.split(',').collect();
    let endpoint = endpoints.first().unwrap(); // TODO: use Promise::race();

    let url = format!("{endpoint}/{pk}/{network}-{name}/metadata");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .map_err(|op| {
            CarbonadoError::StdIoError(Error::new(ErrorKind::Interrupted, op.to_string()))
        })?;

    let status_code = response.status().as_u16();

    if status_code != 200 {
        let response_text = response.text().await.map_err(|_| {
            CarbonadoError::StdIoError(Error::new(
                ErrorKind::Unsupported,
                format!("Error in parsing server response for POST JSON request to {url}"),
            ))
        })?;

        return Err(CarbonadoError::StdIoError(Error::new(
            ErrorKind::Other,
            format!(
                "Error in storing carbonado file, status: {status_code} error: {response_text}"
            ),
        )));
    }

    let result = response.json::<FileMetadata>().await.map_err(|_| {
        CarbonadoError::StdIoError(Error::new(
            ErrorKind::Unsupported,
            format!("Error in parsing server response for POST JSON request to {url}"),
        ))
    })?;

    Ok(result)
}

#[cfg(feature = "server")]
pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let network = NETWORK.read().await.to_string();
    let networks = ["bitcoin", "mainnet", "testnet", "signet", "regtest"];

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

#[cfg(not(feature = "server"))]
async fn server_req(endpoint: &str) -> Result<Option<Vec<u8>>, CarbonadoError> {
    let client = reqwest::Client::new();
    let response = client
        .get(endpoint)
        .header("Accept", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .map_err(|op| {
            CarbonadoError::StdIoError(Error::new(ErrorKind::Interrupted, op.to_string()))
        });

    if let Ok(response) = response {
        let status_code = response.status().as_u16();
        if status_code == 200 {
            let bytes = response.bytes().await.map_err(|_| {
                CarbonadoError::StdIoError(Error::new(
                    ErrorKind::UnexpectedEof,
                    format!("Error in parsing server response for POST JSON request to {endpoint}"),
                ))
            })?;
            return Ok(Some(bytes.to_vec()));
        }
    }

    Ok(None)
}

#[cfg(not(feature = "server"))]
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
    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let name = format!("{network}-{name}");
    if let Some(encoded) = server_req(format!("{endpoint}/{pk}/{name}").as_str())
        .await
        .map_err(|_| {
            CarbonadoError::StdIoError(Error::new(
                ErrorKind::NotFound,
                format!("Cannot create filepath to carbonado file {name}"),
            ))
        })?
    {
        if Header::len() < encoded.len() {
            let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }
    };

    // Check alternative names
    let alt_names = alt_names.into_iter().map(|x| format!("{network}-{x}"));
    for alt_name in alt_names {
        if let Some(encoded) = server_req(format!("{endpoint}/{pk}/{alt_name}").as_str())
            .await
            .map_err(|_| {
                CarbonadoError::StdIoError(Error::new(
                    ErrorKind::NotFound,
                    format!("Cannot create filepath to carbonado file {name}"),
                ))
            })?
        {
            if Header::len() < encoded.len() {
                let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
                return Ok((decoded, header.metadata.map(|m| m.to_vec())));
            }
        };
    }

    Ok((Vec::new(), None))
}

#[cfg(feature = "server")]
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
    let networks = ["bitcoin", "mainnet", "testnet", "signet", "regtest"];
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

#[cfg(feature = "server")]
pub async fn handle_file(pk: &str, name: &str, bytes: usize) -> Result<PathBuf, CarbonadoError> {
    let mut final_name = name.to_string();
    let network = NETWORK.read().await.to_string();
    let networks = ["bitcoin", "mainnet", "testnet", "signet", "regtest"];
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

// Utility functions for handling data of different encodings
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
