use std::str::FromStr;

use anyhow::{anyhow, Result};
use rgb20::Asset;
use rgb_core::{
    data::Revealed,
    vm::embedded::constants::{FIELD_TYPE_NAME, FIELD_TYPE_TICKER},
};
use rgb_std::{Consignment, Contract, Node};

use crate::{
    data::structs::{Allocation, Amount, AssetResponse, ThinAsset},
    info,
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

pub fn get_assets(_contract: &str) -> Result<Vec<AssetResponse>> {
    todo!("decode assets from contract(s)");
}
