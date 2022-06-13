use anyhow::Result;
use gloo_console::log;
use gloo_net::http::Request;

use crate::data::{constants::NODE_SERVER_BASE_URL, structs::ValidateRequest};

pub async fn validate_transfer(consignment: String, node_url: Option<String>) -> Result<()> {
    //TODO: review
    let validate_request = ValidateRequest { consignment };

    let node_url = node_url.unwrap_or(NODE_SERVER_BASE_URL.to_string());
    let url = format!("{}validate", node_url);
    let response = Request::post(&url)
        .body(serde_json::to_string(&validate_request)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await?;

    // parse into generic JSON value
    let js: String = response.json().await?;

    log!(format!("validate_transfer result {js:?}"));
    Ok(())
}
