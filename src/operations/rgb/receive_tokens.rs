use anyhow::{format_err, Result};

use crate::{
    data::{
        constants::url,
        structs::{BlindResponse, OutPoint},
    },
    log,
    util::post_json,
};

pub async fn blind_utxo(
    utxo: OutPoint,
    node_url: Option<String>,
) -> Result<(BlindResponse, OutPoint)> {
    log!("in blind_utxo");
    log!(format!("utxo {utxo:?}"));
    let (response, status) = post_json(url("blind", &node_url), &utxo).await?;
    log!(format!("response status: {status}"));

    if status == 200 {
        // parse into generic JSON value
        let js: BlindResponse = serde_json::from_str(&response)?;
        log!(format!("blind utxo result {js:?}"));
        Ok((js, utxo))
    } else {
        Err(format_err!(
            "Error from blind utxo response. Status: {status}"
        ))
    }
}
