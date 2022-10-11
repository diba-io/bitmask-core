use anyhow::Result;

use crate::{
    data::{constants::get_endpoint, structs::ValidateRequest},
    info,
    util::post_json,
};

#[allow(unused_variables, unreachable_code)]
pub async fn validate_transfer(consignment: String) -> Result<()> {
    //TODO: review
    let validate_request = ValidateRequest { consignment };

    todo!("this code actually loops... there's no implementation");
    let (response, _) = post_json(&get_endpoint("validate").await, &validate_request).await?;

    // parse into generic JSON value
    // let result = serde_json::from_str(&response)?;

    info!(format!("validate_transfer result {response:?}"));
    Ok(())
}
