use anyhow::Result;
use gloo_console::log;
use reqwasm::http::Request;

use crate::data::{
    constants::NODE_SERVER_BASE_URL,
    structs::{BlindResponse, OutPoint},
};

pub async fn blind_utxo(utxo: OutPoint) -> Result<(BlindResponse, OutPoint)> {
    log!("in blind_utxo");
    log!(format!("utxo {:?}", utxo));
    let url = format!("{}blind", *NODE_SERVER_BASE_URL);
    let response = Request::post(&url)
        .body(serde_json::to_string(&utxo)?)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    log!("made");

    // parse into generic JSON value
    let js: BlindResponse = response.json().await?;

    //let person: Person = serde_json::from_str(&js.data)?;
    log!(format!("aaa {:?}", js));
    Ok((js, utxo))
}
