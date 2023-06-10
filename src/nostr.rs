use std::{collections::HashMap, str::FromStr};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    constants::LNDHUB_ENDPOINT,
    util::{get, post_json_auth},
};

/// Nostr pubkey
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Nostr {
    pub pubkey: String,
}

/// Status response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
    pub status: String,
}

/// Add a new nostr pubkey to a user
pub async fn new_nostr_pubkey(pubkey: &str, token: &str) -> Result<Response> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let pubkey = Nostr {
        pubkey: pubkey.to_string(),
    };
    let url = format!("{endpoint}/nostr_pubkey");
    let response = post_json_auth(&url, &Some(pubkey), Some(token)).await?;

    let res: Response = serde_json::from_str(&response)?;

    Ok(res)
}

/// Update the user nostr pubkey
pub async fn update_nostr_pubkey(pubkey: &str, token: &str) -> Result<Response> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let pubkey = Nostr {
        pubkey: pubkey.to_string(),
    };
    let url = format!("{endpoint}/update_nostr_pubkey");
    let response = post_json_auth(&url, &Some(pubkey), Some(token)).await?;

    let res: Response = serde_json::from_str(&response)?;

    Ok(res)
}
