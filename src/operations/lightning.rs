use crate::{
    constants::LNDHUB_ENDPOINT,
    util::{get, post_json},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub login: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tokens {
    pub refresh_token: String,
    pub access_token: String,
}

pub async fn create_wallet() -> Result<Credentials> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let create_url = format!("{endpoint}/create");
    let (response, _) = post_json::<Credentials>(&create_url, &None).await?;
    let creds: Credentials = serde_json::from_str(&response)?;

    Ok(creds)
}

pub async fn auth(creds: Credentials) -> Result<Tokens> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let auth_url = format!("{endpoint}/auth");
    let (response, _) = post_json(&auth_url, &Some(creds)).await?;
    let tokens: Tokens = serde_json::from_str(&response)?;

    Ok(tokens)
}

pub async fn decode_invoice(invoice: &str, token: &str) -> Result<String> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/decodeinvoice?invoice={invoice}");
    let (response, _) = get(&url, token).await?;
    // TODO: return an invoice struct

    Ok(response)
}
