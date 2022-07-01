use anyhow::Result;

use crate::{
    data::{constants::url, structs::ValidateRequest},
    log,
    util::post_json,
};

pub async fn validate_transfer(consignment: String, node_url: Option<String>) -> Result<()> {
    //TODO: review
    let validate_request = ValidateRequest { consignment };

    let (response, _) = post_json(url("validate", &node_url), &validate_request).await?;

    // parse into generic JSON value
    // let result = serde_json::from_str(&response)?;

    log!(format!("validate_transfer result {response:?}"));
    Ok(())
}
