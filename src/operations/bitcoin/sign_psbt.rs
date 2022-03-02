use anyhow::{Error, Result};
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SignOptions, Wallet};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction};
use gloo_console::log;

pub async fn sign_psbt(
    wallet: &Wallet<EsploraBlockchain, MemoryDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<()> {
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    if finalized {
        log!("Signed PSBT: {}", base64::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        log!("tx: {}", base64::encode(&serialize(&tx)));
        wallet.broadcast(&tx).await?;
        Ok(())
    } else {
        Err(Error::msg(""))
    }
}
