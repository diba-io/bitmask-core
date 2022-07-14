use std::{collections::BTreeMap, str::FromStr};

use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use lnpbp::chain::Chain;
use rgb20::{Asset, Rgb20};
use rgb_core::{value::Revealed, Genesis};
use rgb_std::{
    fungible::allocation::OutpointValue, AssignedState, Consignment, Contract, OwnedValue,
};
use stens::AsciiString;

use crate::{constants::NETWORK, log};

fn ticker_validator(name: &str) -> Result<()> {
    log!(format!("validating ticker name: {name}"));
    if name.len() < 3 || name.len() > 8 || name.chars().any(|c| c < 'A' && c > 'Z') {
        Err(anyhow!("Ticker name must be between 3 and 8 chars, contain no spaces and consist only of capital letters".to_string()))
    } else {
        Ok(())
    }
}

pub fn issue_asset(
    ticker: &str,
    name: &str,
    precision: u8,
    supply: u64,
    utxo: OutPoint,
) -> Result<(Genesis, Vec<OwnedValue>)> {
    ticker_validator(ticker)?;

    let network = Chain::from(*NETWORK.read().unwrap());
    let ticker = AsciiString::try_from(ticker)?;
    let name = AsciiString::try_from(name)?;
    let outpoint_value = format!("{supply}@{utxo}");
    let allocation = vec![OutpointValue::from_str(&outpoint_value)?];

    let contract = Contract::create_rgb20(
        network,
        ticker,
        name,
        precision,
        allocation,
        BTreeMap::new(),
        None,
        None,
    );

    let asset = Asset::try_from(&contract)?;
    let genesis = contract.genesis();
    let known_coins: Vec<AssignedState<Revealed>> = asset.known_coins().cloned().collect();

    let genesis_json = serde_json::to_string(genesis)?;
    // let known_coins_json = serde_json::to_string(known_coins)?;

    log!(format!("genesis: {genesis_json}"));
    // log!(format!("known coins: {known_coins_json}"));

    Ok((genesis.to_owned(), known_coins))
}
