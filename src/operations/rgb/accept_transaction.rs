use anyhow::Result;
use electrum_client::Client;
use lnpbp::bech32::FromBech32Str;
use rgb_core::Validator;
use rgb_std::{validation::Status, InmemConsignment};

use crate::{data::constants::BITCOIN_ELECTRUM_API, debug, info};

pub async fn accept_transfer(consignment: &str) -> Result<(String, Status)> {
    let url = BITCOIN_ELECTRUM_API.read().await;
    let electrum_client = Client::new(&url)?;
    debug!(format!("Electrum client connected to {url}"));

    let consignment = InmemConsignment::from_bech32_str(consignment)?;
    let status = Validator::validate(&consignment, &electrum_client);
    info!(format!("accept transfer result: {status:?}"));
    let id = consignment.contract_id().to_string();

    Ok((id, status))
}
