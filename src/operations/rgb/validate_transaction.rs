use anyhow::Result;
use gloo_console::log;
use gloo_net::http::Request;

use crate::data::{constants::url, structs::ValidateRequest};

pub async fn validate_transfer(consignment: String, node_url: Option<String>) -> Result<()> {
    //TODO: review
    let validate_request = ValidateRequest { consignment };

    let response = Request::post(&url("validate", &node_url))
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
