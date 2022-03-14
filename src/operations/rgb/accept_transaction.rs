use anyhow::{Context, Result};
use gloo_console::log;
use gloo_net::http::Request;

use crate::{
    data::{constants::NODE_SERVER_BASE_URL, structs::AcceptRequest},
    OutPoint,
};

pub async fn accept_transfer(
    consignment: String,
    outpoint: OutPoint,
    blinding_factor: u64,
) -> Result<String> {
    let accept_request = AcceptRequest {
        consignment,
        outpoint,
        blinding_factor,
    };
    log!("here);");
    let url = format!("{}accept", *NODE_SERVER_BASE_URL);
    let response = Request::post(&url)
        .body(serde_json::to_string(&accept_request)?)
        .header("Content-Type", "application/json")
        .send()
        .await
        .context("Problem at receiving request")?;
    log!("response");
    // parse into generic JSON value
    let js: String = response
        .json()
        .await
        .context("Problem at serdering servor response")?; //TODO: not working
    log!("json");
    log!(&js);

    log!(format!("accept transfer result: {js:?}"));
    Ok(js)
}
