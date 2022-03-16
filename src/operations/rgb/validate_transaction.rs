use anyhow::Result;
use gloo_console::log;
use gloo_net::http::Request;

use crate::data::{constants::NODE_SERVER_BASE_URL, structs::ValidateRequest};

pub async fn validate_transfer(consignment: String) -> Result<()> {
    //TODO: review
    let validate_request = ValidateRequest { consignment };

    let url = format!("{}validate", *NODE_SERVER_BASE_URL);
    let response = Request::post(&url)
        .body(serde_json::to_string(&validate_request)?)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    // parse into generic JSON value
    let js: String = response.json().await?;

    log!(format!("validate_transfer result {js:?}"));
    Ok(())
}
