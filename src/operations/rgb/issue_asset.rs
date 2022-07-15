use std::{collections::BTreeMap, str::FromStr};

use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use lnpbp::chain::Chain;
use rgb20::Rgb20;
use rgb_core::Consignment;
use rgb_std::{fungible::allocation::OutpointValue, Contract};
use stens::AsciiString;

use crate::{constants::NETWORK, debug, info};

fn ticker_validator(ticker: &str) -> Result<()> {
    info!(format!("Validating ticker: {ticker}"));
    if ticker.len() < 3 || ticker.len() > 8 || ticker.chars().any(|c| c < 'A' && c > 'Z') {
        Err(anyhow!("Ticker must be between 3 and 8 chars, contain no spaces and consist only of capital letters".to_string()))
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
) -> Result<Contract> {
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

    debug!(format!("Contract genesis: {:#?}", contract.genesis()));

    Ok(contract)
}
