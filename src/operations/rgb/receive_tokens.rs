use anyhow::Result;
use bitcoin::OutPoint;
use bp::seals::txout::CloseMethod;
use rgb_core::seal;

use crate::{data::structs::BlindResponse, info};

pub fn blind_utxo(utxo: OutPoint) -> Result<BlindResponse> {
    let seal = seal::Revealed::new(CloseMethod::TapretFirst, utxo);

    let result = BlindResponse {
        blinding: seal.blinding.to_string(),
        conceal: seal.to_concealed_seal().to_string(),
    };

    info!(format!("Blind result: {result:#?}"));

    Ok(result)
}
