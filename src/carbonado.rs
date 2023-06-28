#[cfg(feature = "server")]
use crate::info;
#[cfg(feature = "server")]
use tokio::fs;

use amplify::hex::ToHex;
use anyhow::Result;
#[cfg(not(feature = "server"))]
use anyhow::{anyhow, Context};
use bitcoin_30::secp256k1::{PublicKey, SecretKey};
use carbonado::file::Header;
#[cfg(not(feature = "server"))]
use percent_encoding::utf8_percent_encode;

pub mod constants;

use crate::structs::FileMetadata;
#[cfg(not(feature = "server"))]
use crate::{
    carbonado::constants::FORM,
    constants::{CARBONADO_ENDPOINT, NETWORK},
};

#[cfg(not(feature = "server"))]
pub async fn store(sk: &str, name: &str, input: &[u8]) -> Result<()> {
    let level = 15;
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.serialize();
    let pk_hex = hex::encode(pk);
    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, None)?;
    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let name = utf8_percent_encode(name, FORM);
    let network = NETWORK.read().await.to_string();
    let url = format!("{endpoint}/{pk_hex}/{network}-{name}");
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .body(body)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    if status_code != 200 {
        let response_text = response.text().await.context(format!(
            "Error in parsing server response for POST JSON request to {url}"
        ))?;

        Err(anyhow!(
            "Error in storing carbonado file, status: {status_code} error: {response_text}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(feature = "server")]
pub async fn store(sk: &str, name: &str, input: &[u8]) -> Result<()> {
    let level = 15;
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.serialize();
    let pk_hex = hex::encode(pk);
    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, None)?;

    let filepath = handle_file(&pk_hex, name, body.len()).await?;
    fs::write(filepath, body).await?;
    Ok(())
}

#[cfg(not(feature = "server"))]
pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata> {
    let mut result = FileMetadata::default();
    let (header, _) = retrieve(sk, name).await?;
    if let Some(header) = header {
        result.filename = header.file_name();
        result.metadata = header.metadata.to_string();
    }

    Ok(result)
}

#[cfg(not(feature = "server"))]
pub async fn retrieve(sk: &str, name: &str) -> Result<Vec<u8>> {
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let name = utf8_percent_encode(name, FORM);
    let network = NETWORK.read().await.to_string();
    let url = format!("{endpoint}/{pk}/{network}-{name}");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    if status_code != 200 {
        let response_text = response.text().await.context(format!(
            "Error in parsing server response for POST JSON request to {url}"
        ))?;
        return Err(anyhow!(
            "Error in retrieving carbonado file, status: {status_code} error: {response_text}"
        ));
    }

    if encoded.is_empty() {
        Ok((None, Vec::new()))
    } else {
        let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
        Ok((Some(header), decoded))
    }
}

#[cfg(feature = "server")]
pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata> {
    let mut result = FileMetadata::default();
    let (header, _) = retrieve(sk, name).await?;
    if let Some(header) = header {
        result.filename = header.file_name();
        result.metadata = header.metadata.to_string();
    }

    Ok(result)
}

#[cfg(feature = "server")]
pub async fn retrieve(sk: &str, name: &str) -> Result<(Option<Header>, Vec<u8>)> {
    use crate::constants::NETWORK;

    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let network = NETWORK.read().await.to_string();
    let mut final_name = name.to_string();
    if !name.contains(&network) {
        final_name = format!("{network}-{name}");
    }

    let filepath = handle_file(&pk, &final_name, 0).await?;
    let bytes = fs::read(filepath).await?;

    if bytes.is_empty() {
        Ok((None, Vec::new()))
    } else {
        let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
        Ok((Some(header), decoded))
    }
}

#[cfg(feature = "server")]
pub async fn handle_file(pk: &str, name: &str, bytes: usize) -> Result<std::path::PathBuf> {
    use crate::constants::NETWORK;

    let network = NETWORK.read().await.to_string();
    let mut final_name = name.to_string();
    if !name.contains(&network) {
        final_name = format!("{network}-{name}");
    }

    let directory = std::path::Path::new(
        &std::env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned()),
    )
    .join(pk);

    let filepath = directory.join(final_name);
    fs::create_dir_all(directory).await?;
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

pub fn decode_hex(string: &str) -> Result<Vec<u8>> {
    Ok(hex::decode(string)?)
}

pub fn decode_base64(string: &str) -> Result<Vec<u8>> {
    Ok(base64::decode(string)?)
}
