use anyhow::Result;
use electrum_client::Client;
use rgb_core::Validator;
use rgb_std::{validation::Status, InmemConsignment, TransferConsignment};
use strict_encoding::strict_deserialize;

use crate::{data::constants::BITCOIN_ELECTRUM_API, debug, info, util::bech32m_zip_decode};

pub async fn accept_transfer(consignment: &str) -> Result<(String, Status)> {
    let url = BITCOIN_ELECTRUM_API.read().await;
    let electrum_client = Client::new(&url)?;
    debug!(format!("Electrum client connected to {url}"));

    let consignment = bech32m_zip_decode(consignment)?;
    let consignment: InmemConsignment<TransferConsignment> = strict_deserialize(consignment)?;
    // let consignment = InmemConsignment::from_bech32_str(consignment)?;
    let status = Validator::validate(&consignment, &electrum_client);
    info!(format!("accept transfer result: {status:?}"));
    let id = consignment.contract_id().to_string();

    Ok((id, status))
}
