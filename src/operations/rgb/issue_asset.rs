use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use bp::seals::txout::CloseMethod::TapretFirst;
use lnpbp::chain::Chain;
use rgb20::Rgb20;
use rgb121::Rgb121;
use rgb_core::Consignment;
use rgb_std::{fungible::allocation::OutpointValue, Contract};
use stens::AsciiString;

use crate::{data::constants::NETWORK, debug, info};

use super::shared::ContractType;

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
    contract_type: ContractType
) -> Result<Contract> {
    ticker_validator(ticker)?;

    let network = Chain::from(*NETWORK.read().unwrap());
    let ticker = AsciiString::try_from(ticker)?;
    let name = AsciiString::try_from(name)?;
    let allocations = vec![OutpointValue {
        value: supply,
        outpoint: utxo,
    }];

    let contract = match contract_type {
        ContractType::Fungible => Contract::create_rgb20(
            network,
            ticker,
            name,
            precision,
            allocations,
            BTreeMap::new(),
            TapretFirst,
            None,
            None,
        ),
        ContractType::UDA => Contract::create_rgb121(
            network,
            name,
            None,
            precision,
            None,
            vec![],
            vec![],
            allocations,
            TapretFirst,
        )?
    };

    debug!(format!("Contract genesis: {:#?}", contract.genesis()));
    info!(format!("Contract: {}", contract));

    Ok(contract)
}
