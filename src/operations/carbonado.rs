use amplify::hex::ToHex;
use anyhow::{anyhow, Context, Result};
use bitcoin_30::secp256k1::{PublicKey, SecretKey};

use crate::data::constants::CARBONADO_ENDPOINT;

pub async fn store(sk: &str, input: &[u8]) -> Result<()> {
    let level = 15;
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.serialize();

    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level)?;

    let url = CARBONADO_ENDPOINT.read().await.to_string();
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .body(body)
        .header("Content-Type", "application/octet-stream")
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

pub async fn retrieve(sk: &str) -> Result<Vec<u8>> {
    let sk = hex::decode(sk)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let url = format!("{endpoint}/{pk}.c15");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/octet-stream")
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

    let encoded = response.bytes().await?;
    let (_header, decoded) = carbonado::file::decode(&sk, &encoded)?;

    Ok(decoded)
}
