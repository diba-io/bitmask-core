use anyhow::Result;

use crate::{
    constants::ACCEPT_TRANSFER_ENDPOINT, data::structs::AcceptRequest, info, util::post_json,
    OutPoint,
};

#[allow(dead_code)]
pub async fn accept_transfer(
    consignment: String,
    outpoint: OutPoint,
    blinding_factor: String,
) -> Result<String> {
    let accept_request = AcceptRequest {
        consignment,
        outpoint,
        blinding_factor,
    };
    info!("here);");
    let (response, _) = post_json(&ACCEPT_TRANSFER_ENDPOINT, &accept_request).await?;
    info!(format!("accept transfer result: {response:?}"));
    Ok(response)
}
