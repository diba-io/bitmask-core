use anyhow::{format_err, Result};
use gloo_console::log;
use gloo_net::http::Request;

use crate::data::{
    constants::url,
    structs::{BlindResponse, OutPoint},
};

pub async fn blind_utxo(
    utxo: OutPoint,
    node_url: Option<String>,
) -> Result<(BlindResponse, OutPoint)> {
    log!("in blind_utxo");
    log!(format!("utxo {utxo:?}"));
    let response = Request::post(&url("blind", &node_url))
        .body(serde_json::to_string(&utxo)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await?;
    log!("made");

    let status = response.status();

    if status == 200 {
        // parse into generic JSON value
        let js: BlindResponse = response.json().await?;

        //let person: Person = serde_json::from_str(&js.data)?;
        log!(format!("blind utxo result {js:?}"));
        Ok((js, utxo))
    } else {
        Err(format_err!(
            "Error from blind utxo response. Status: {status}"
        ))
    }
}
