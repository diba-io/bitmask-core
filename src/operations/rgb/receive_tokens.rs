use anyhow::Result;
use bitcoin::OutPoint;
use commit_verify::commit_encode::CommitConceal;
use rgb::seal;

use crate::{data::structs::BlindResponse, log};

pub fn blind_utxo(utxo: OutPoint) -> Result<(BlindResponse, OutPoint)> {
    let seal = seal::Revealed::from(utxo);

    let result = BlindResponse {
        blinding: seal.blinding.to_string(),
        conceal: seal.commit_conceal().to_string(),
    };

    log!(format!("blind result: {result:?}"));

    Ok((result, utxo))
}
