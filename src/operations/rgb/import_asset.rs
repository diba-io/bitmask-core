use std::{collections::HashMap, str::FromStr};

use anyhow::{Error, Result};
use gloo_console::log;
use reqwasm::http::Request;
use serde::{Deserialize, Serialize};

use crate::data::{
    constants::NODE_SERVER_BASE_URL,
    structs::{Allocation, Asset, ExportRequestMini, ThinAsset},
};

trait FromString {
    fn from_string(str: String) -> serde_json::Value;
}

impl FromString for serde_json::Value {
    fn from_string(str: String) -> serde_json::Value {
        serde_json::Value::from_str(&str).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Person {
    first_name: String,
    last_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersonResponse {
    data: String,
    method: String,
    headers: HashMap<String, String>,
}

// fn print_type_of<T>(_: &T) {
//     log!("{}", std::any::type_name::<T>())
// }

pub async fn get_asset(
    asset: Option<String>,
    _genesis: Option<String>,
    unspent: Vec<bdk::LocalUtxo>,
) -> Result<ThinAsset> {
    let asset_data = ExportRequestMini {
        asset: asset.clone().unwrap(),
    };
    let url = format!("{}getasset", *NODE_SERVER_BASE_URL);
    log!(format!("url: {url:#?}"));
    let response = match Request::post(&url)
        .body(serde_json::to_string(&asset_data)?)
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => return Err(Error::msg(e)),
    };
    log!(format!("response: {response:#?}"));
    let assets: Vec<Asset> = response.json().await?;
    if assets.is_empty() {
        return Err(Error::msg("Incorrect rgb id".to_string()));
    }
    let allocations: Vec<Allocation> = assets[0]
        .known_allocations
        .clone()
        .into_iter()
        .filter(|a| {
            unspent
                .clone()
                .into_iter()
                .any(|y| y.outpoint.to_string().eq(&a.outpoint))
        })
        .collect();
    log!(format!("allocations: {allocations:#?}"));
    let amount = allocations
        .clone()
        .into_iter()
        .map(|a| a.revealed_amount.value)
        .reduce(|a, b| a + b);
    log!(format!("amount: {amount:#?}"));
    let thin_assets = ThinAsset {
        id: asset.unwrap(),
        ticker: assets[0].ticker.clone(),
        name: assets[0].name.clone(),
        description: assets[0].description.clone().unwrap(),
        allocations,
        balance: Some(amount.unwrap_or_default()),
        dolar_balance: None,
    };

    log!(format!("thin_assets: {thin_assets:?}"));
    Ok(thin_assets)
}

pub async fn get_assets() -> Result<Vec<Asset>> {
    let url = format!("{}list", *NODE_SERVER_BASE_URL);
    log!(format!("url list assets: {url:#?}"));
    let response = Request::get(&url).send().await?;
    log!(format!("listassets: {response:#?}"));
    let assets: Vec<Asset> = response.json().await?;
    Ok(assets)
}
