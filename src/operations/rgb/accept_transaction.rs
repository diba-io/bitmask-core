use anyhow::Result;

use crate::{
    data::{constants::url, structs::AcceptRequest},
    log,
    util::post_json,
    OutPoint,
};

pub async fn accept_transfer(
    consignment: String,
    outpoint: OutPoint,
    blinding_factor: String,
    node_url: Option<String>,
) -> Result<String> {
    let accept_request = AcceptRequest {
        consignment,
        outpoint,
        blinding_factor,
    };
    log!("here);");
    let (response, _) = post_json(url("accept", &node_url), &accept_request).await?;
    log!(format!("accept transfer result: {response:?}"));
    Ok(response)
}
