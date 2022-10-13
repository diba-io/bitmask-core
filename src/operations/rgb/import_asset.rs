use std::str::FromStr;

use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use rgb20::Asset;
use rgb_core::{
    data::Revealed as DataRevealed,
    vm::embedded::constants::{FIELD_TYPE_NAME, FIELD_TYPE_TICKER},
};
use rgb_std::{Consignment, Contract, Node};

use crate::{
    data::structs::{Allocation, Amount, AssetResponse, ThinAsset},
    info,
};

pub fn get_allocations(asset: Asset, asset_utxos: &Vec<OutPoint>) -> (u64, Vec<Allocation>) {
    let mut allocations = vec![];
    let mut balance = 0;
    let mut index = 0;

    for outpoint in asset_utxos {
        let coins = asset.outpoint_coins(*outpoint);
        for coin in coins.iter() {
            let outpoint = coin.outpoint.to_string();
            let node_id = coin.outpoint.node_id.to_string();
            let seal_txid = coin.seal.txid.to_string();
            let seal_vout = coin.seal.vout;

            let blinding = coin.state.blinding.to_string();
            let value = coin.state.value;
            let amount = Amount { value, blinding };

            balance += coin.state.value;
            index += 1;

            allocations.push(Allocation {
                index,
                outpoint,
                node_id,
                seal_txid,
                seal_vout,
                amount,
            });
        }
    }

    (balance, allocations)
}

pub fn get_asset_by_genesis(genesis: &str, asset_utxos: &Vec<OutPoint>) -> Result<ThinAsset> {
    let contract = Contract::from_str(genesis)?;
    let asset = Asset::try_from(&contract)?;

    let id = contract.contract_id().to_string();
    let metadata = contract.genesis().metadata();

    let ticker = match metadata.get(&FIELD_TYPE_TICKER).unwrap().get(0) {
        Some(DataRevealed::AsciiString(ticker)) => ticker.to_string(),
        _ => return Err(anyhow!("Error decoding asset ticker")),
    };
    let name = match metadata.get(&FIELD_TYPE_NAME).unwrap().get(0) {
        Some(DataRevealed::AsciiString(name)) => name.to_string(),
        _ => return Err(anyhow!("Error decoding asset name")),
    };

    let (balance, allocations) = get_allocations(asset, asset_utxos);

    let asset = ThinAsset {
        id,
        ticker,
        name,
        allocations,
        balance,
        genesis: genesis.to_owned(),
    };

    info!(format!("Asset decoded from genesis: {asset:#?}"));

    Ok(asset)
}

pub fn get_assets(_contract: &str) -> Result<Vec<AssetResponse>> {
    todo!("decode assets from contract(s)");
}
