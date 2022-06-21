use anyhow::{Error, Result};
use bdk::{blockchain::Blockchain, database::MemoryDatabase, SignOptions, Wallet};
use bdk_macros::maybe_await;
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction};
use gloo_console::log;

use crate::operations::bitcoin::balance::get_blockchain;

pub async fn sign_psbt(
    wallet: &Wallet<MemoryDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<()> {
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    if finalized {
        log!("Signed PSBT: {}", base64::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        log!("tx: {}", base64::encode(&serialize(&tx)));
        let blockchain = get_blockchain();
        maybe_await!(blockchain.broadcast(&tx))?;
        Ok(())
    } else {
        Err(Error::msg(""))
    }
}
