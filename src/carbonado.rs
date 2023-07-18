#[cfg(feature = "server")]
use crate::info;
#[cfg(feature = "server")]
use tokio::fs;

use amplify::hex::ToHex;
use bitcoin_30::secp256k1::{PublicKey, SecretKey};
#[cfg(not(feature = "server"))]
use percent_encoding::utf8_percent_encode;
use std::io::{Error, ErrorKind};
#[cfg(feature = "server")]
use std::path::PathBuf;
pub mod constants;
pub mod error;

use crate::{carbonado::error::CarbonadoError, constants::NETWORK, structs::FileMetadata};

#[cfg(not(feature = "server"))]
use crate::{carbonado::constants::FORM, constants::CARBONADO_ENDPOINT};

#[cfg(not(feature = "server"))]
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

    let meta: Option<[u8; 8]> = metadata.map(|m| m.try_into().expect("invalid metadata size"));
    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let name = utf8_percent_encode(name, FORM);
    let network = NETWORK.read().await.to_string();

    let mut force_write = "";
    if force {
        force_write = "/force";
    }

    let url = format!("{endpoint}/{pk_hex}/");
    let param = format!("{network}/{name}");
    let query_param = utf8_percent_encode(&param, FORM);

    let url = format!("{url}{query_param}{force_write}");
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .body(body)
        .header("Content-Type", "application/octet-stream")
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

        Err(CarbonadoError::StdIoError(Error::new(
            ErrorKind::Other,
            format!(
                "Error in storing carbonado file, status: {status_code} error: {response_text}"
            ),
        )))
    } else {
        Ok(())
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
    use percent_encoding::percent_decode;

    let level = 15;
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.serialize();
    let pk_hex = hex::encode(pk);

    let meta: Option<[u8; 8]> = metadata.map(|m| m.try_into().expect("invalid metadata size"));

    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level, meta)?;
    let final_name = percent_decode(name.as_bytes()).decode_utf8().unwrap();
    let filepath = handle_file(&pk_hex, &final_name, body.len()).await?;
    fs::write(filepath, body).await?;
    Ok(())
}

#[cfg(not(feature = "server"))]
pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let name = utf8_percent_encode(name, FORM);
    let network = NETWORK.read().await.to_string();
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
    use percent_encoding::percent_decode;

    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let network = NETWORK.read().await.to_string();
    let mut final_name = name.to_string();
    if !name.contains(&network) {
        final_name = format!("{network}/{name}");
    }

    let final_name = percent_decode(final_name.as_bytes()).decode_utf8().unwrap();
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
    let url = format!("{endpoint}/{pk}/");
    let param = format!("{network}/{name}");
    let query_param = utf8_percent_encode(&param, FORM);

    if let Some(encoded) = server_req(format!("{url}{query_param}").as_str())
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
        let url = format!("{endpoint}/{pk}/");
        let query_param = utf8_percent_encode(&alt_name, FORM);

        if let Some(encoded) = server_req(format!("{url}{query_param}").as_str())
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
    use percent_encoding::percent_decode;

    use crate::rgb::constants::RGB_STRICT_TYPE_VERSION;

    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let network = NETWORK.read().await.to_string();
    let mut final_name = name.to_string();
    if !name.contains(&network) {
        final_name = format!("{network}/{name}");
    }

    let final_name = percent_decode(final_name.as_bytes()).decode_utf8().unwrap();
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
    use percent_encoding::percent_decode;

    let network = NETWORK.read().await.to_string();
    let mut final_name = name.to_string();
    if !name.contains(&network) {
        final_name = format!("{network}/{name}");
    }

    let directory = std::path::Path::new(
        &std::env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned()),
    )
    .join(pk);

    let final_name = percent_decode(final_name.as_bytes()).decode_utf8().unwrap();
    let filepath = directory.join(final_name.to_string());
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
