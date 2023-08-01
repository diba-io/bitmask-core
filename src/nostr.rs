use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{constants::LNDHUB_ENDPOINT, util::post_json_auth};

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

fn validate_pubkey(pubkey: &str) -> Result<String> {
    let pubkey = hex::decode(pubkey)?;

    if pubkey.len() == 32 {
        Ok(hex::encode(pubkey))
    } else if pubkey.len() == 33 && (pubkey[0] == 2 || pubkey[0] == 3) {
        Ok(hex::encode(pubkey.get(1..33).unwrap()))
    } else {
        Err(anyhow!("Hex key is of wrong length or format"))
    }
}

#[test]
fn test_validate_pubkey() -> Result<()> {
    let result =
        validate_pubkey("03b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9")?;

    assert_eq!(
        "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9", result,
        "strips leading parity byte on 33 byte x-only pubkey"
    );

    Ok(())
}

/// Add a new nostr pubkey to a user
pub async fn new_nostr_pubkey(pubkey: &str, token: &str) -> Result<Response> {
    let pubkey = validate_pubkey(pubkey)?;

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
    let pubkey = validate_pubkey(pubkey)?;

    let endpoint = LNDHUB_ENDPOINT.read().await;
    let pubkey = Nostr {
        pubkey: pubkey.to_string(),
    };
    let url = format!("{endpoint}/update_nostr_pubkey");
    let response = post_json_auth(&url, &Some(pubkey), Some(token)).await?;

    let res: Response = serde_json::from_str(&response)?;

    Ok(res)
}
