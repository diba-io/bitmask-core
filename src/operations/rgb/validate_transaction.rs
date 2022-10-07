use anyhow::Result;

use crate::{
    constants::VALIDATE_TRANSFER_ENDPOINT, data::structs::ValidateRequest, info, util::post_json,
};

pub async fn validate_transfer(consignment: String) -> Result<()> {
    //TODO: review
    let validate_request = ValidateRequest { consignment };

    let (response, _) = post_json(&VALIDATE_TRANSFER_ENDPOINT, &Some(validate_request)).await?;

    // parse into generic JSON value
    // let result = serde_json::from_str(&response)?;

    info!(format!("validate_transfer result {response:?}"));
    Ok(())
}
