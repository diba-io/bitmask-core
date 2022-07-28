use std::str::FromStr;

use anyhow::{anyhow, Error, Result};
use rgb20::Asset;
use rgb_core::{
    data::Revealed,
    vm::embedded::constants::{FIELD_TYPE_NAME, FIELD_TYPE_TICKER},
};
use rgb_std::{Consignment, Contract, Node};

use crate::{
    data::{
        constants::url,
        structs::{Allocation, Amount, AssetResponse, ExportRequestMini, ThinAsset},
    },
    info,
    util::{get, post_json},
};

pub fn get_asset_by_genesis(genesis: &str) -> Result<ThinAsset> {
    let contract = Contract::from_str(genesis)?;
    let asset = Asset::try_from(&contract)?;

    let id = contract.contract_id().to_string();
    let metadata = contract.genesis().metadata();

    let ticker = match metadata.get(&FIELD_TYPE_TICKER).unwrap().get(0) {
        Some(Revealed::AsciiString(ticker)) => ticker.to_string(),
        _ => return Err(anyhow!("Error decoding asset ticker")),
    };
    let name = match metadata.get(&FIELD_TYPE_NAME).unwrap().get(0) {
        Some(Revealed::AsciiString(name)) => name.to_string(),
        _ => return Err(anyhow!("Error decoding asset name")),
    };

    // let allocations =
    let allocations: Vec<Allocation> = asset
        .known_coins()
        .enumerate()
        .map(|(index, coin)| {
            let index = index as u32;
            let outpoint = coin.outpoint.to_string();
            let node_id = coin.outpoint.node_id.to_string();
            let seal_txid = coin.seal.txid.to_string();
            let seal_vout = coin.seal.vout;

            let blinding = coin.state.blinding.to_string();
            let value = coin.state.value;
            let amount = Amount { value, blinding };

            Allocation {
                index,
                outpoint,
                node_id,
                seal_txid,
                seal_vout,
                amount,
            }
        })
        .collect();

    let balance = allocations
        .iter()
        .fold(0, |balance, alloc| balance + alloc.amount.value);

    let asset = ThinAsset {
        id,
        ticker,
        name,
        description: "Unlisted asset".to_owned(),
        allocations,
        balance,
    };

    info!(format!("Asset decoded from genesis: {asset:#?}"));

    Ok(asset)
}

pub async fn get_asset_by_contract_id(
    asset: &str,
    unspent: Vec<bdk::LocalUtxo>,
    node_url: Option<String>,
) -> Result<ThinAsset> {
    let asset_data = ExportRequestMini {
        asset: asset.to_owned(),
    };
    let (response, _) = match post_json(url("getasset", &node_url), &asset_data).await {
        Ok(response) => response,
        Err(e) => return Err(Error::msg(e)),
    };
    info!(format!("response: {response:#?}"));
    let assets: Vec<AssetResponse> = serde_json::from_str(&response)?;
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
    info!(format!("allocations: {allocations:#?}"));
    let amount = allocations
        .clone()
        .into_iter()
        .map(|a| a.amount.value)
        .reduce(|a, b| a + b);
    info!(format!("amount: {amount:#?}"));
    let thin_assets = ThinAsset {
        id: asset.to_owned(),
        ticker: assets[0].ticker.clone(),
        name: assets[0].name.clone(),
        description: assets[0].description.clone().unwrap(),
        allocations,
        balance: amount.unwrap_or_default(),
    };

    info!(format!("thin_assets: {thin_assets:?}"));
    Ok(thin_assets)
}

pub async fn get_assets(node_url: Option<String>) -> Result<Vec<AssetResponse>> {
    let (response, _) = get(url("list", &node_url)).await?;
    info!(format!("listassets: {response:#?}"));
    let assets: Vec<AssetResponse> = serde_json::from_str(&response)?;
    Ok(assets)
}
