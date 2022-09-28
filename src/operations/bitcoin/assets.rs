use anyhow::Result;
use bdk::{database::AnyDatabase, wallet::AddressIndex, SignOptions, Wallet};
use bitcoin::psbt::PartiallySignedTransaction;

use crate::debug;

pub fn dust_tx(
    btc_wallet: &Wallet<AnyDatabase>,
    assets_wallet: &Wallet<AnyDatabase>,
    fee_rate: f32,
) -> Result<PartiallySignedTransaction> {
    let dust_amt = if fee_rate < 3.0 {
        546
    } else {
        (182.0 * fee_rate).ceil() as u64
    };

    let assets_address = assets_wallet.get_address(AddressIndex::New)?;

    let mut tx_builder = btc_wallet.build_tx();
    tx_builder.add_recipient(assets_address.script_pubkey(), dust_amt);
    let (mut dust_psbt, _) = tx_builder.finish()?;

    let finalized = btc_wallet.sign(&mut dust_psbt, SignOptions::default())?;
    debug!(format!("PSBT signed. Finalized: {finalized}"));
    btc_wallet.finalize_psbt(&mut dust_psbt, SignOptions::default())?;
    debug!(format!("PSBT finalized. Finalized: {finalized}"));

    Ok(dust_psbt)
}
