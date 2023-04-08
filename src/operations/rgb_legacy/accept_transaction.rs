use std::str::FromStr;

use anyhow::Result;
use bp_legacy::seals::txout::CloseMethod;
use commit_verify::CommitConceal;
use electrum_client::Client;
use rgb_core::{seal::Revealed, Consignment, Validator};
use rgb_std::{validation::Status, InmemConsignment, TransferConsignment};
use strict_encoding_legacy::strict_deserialize;

use crate::{
    data::constants::BITCOIN_ELECTRUM_API, debug, info, rgb_legacy::legacy::Reveal,
    util::bech32m_zip_decode,
};

pub async fn accept_transfer(
    consignment: &str,
    blinding_factor: &str,
    outpoint: &str,
) -> Result<(String, Status, bool)> {
    let url = BITCOIN_ELECTRUM_API.read().await;
    let electrum_client = Client::new(&url)?;
    debug!(format!("Electrum client connected to {url}"));

    let consignment = bech32m_zip_decode(consignment)?;
    let consignment: InmemConsignment<TransferConsignment> = strict_deserialize(consignment)?;
    // let consignment = InmemConsignment::from_bech32_str(consignment)?;
    let status = Validator::validate(&consignment, &electrum_client);
    info!(format!("accept transfer result: {status:?}"));
    let id = consignment.contract_id().to_string();

    let reveal = Reveal {
        blinding_factor: blinding_factor.parse::<u64>()?,
        outpoint: bitcoin::OutPoint::from_str(outpoint)?,
        close_method: CloseMethod::TapretFirst,
    };

    let reveal_outpoint = Revealed {
        method: reveal.close_method,
        blinding: reveal.blinding_factor,
        txid: Some(reveal.outpoint.txid),
        vout: reveal.outpoint.vout,
    };

    let concealed_seals = consignment
        .endpoints()
        .filter(|&&(_, seal)| reveal_outpoint.to_concealed_seal() == seal.commit_conceal())
        .clone();

    Ok((id, status, concealed_seals.count() != 0))
}
