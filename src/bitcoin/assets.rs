use anyhow::Result;
use bdk::{database::AnyDatabase, LocalUtxo, SignOptions, Wallet};
use bitcoin::psbt::PartiallySignedTransaction;

use crate::debug;

pub fn dust_tx(
    btc_wallet: &Wallet<AnyDatabase>,
    fee_rate: f32,
    first_utxo: Option<&LocalUtxo>,
) -> Result<PartiallySignedTransaction> {
    let dust_amt = if fee_rate < 3.0 {
        546
    } else {
        (182.0 * fee_rate).ceil() as u64
    };

    let pubkey = match first_utxo {
        Some(utxo) => utxo.txout.script_pubkey.to_owned(),
        None => todo!(),
    };

    let mut tx_builder = btc_wallet.build_tx();
    tx_builder.add_recipient(pubkey, dust_amt);
    let (mut dust_psbt, tx_details) = tx_builder.finish()?;

    debug!(format!("dust tx details: {tx_details:#?}"));
    let finalized = btc_wallet.sign(&mut dust_psbt, SignOptions::default())?;
    debug!(format!("PSBT signed. Finalized: {finalized}"));
    btc_wallet.finalize_psbt(&mut dust_psbt, SignOptions::default())?;
    debug!(format!("PSBT finalized. Finalized: {finalized}"));

    Ok(dust_psbt)
}
