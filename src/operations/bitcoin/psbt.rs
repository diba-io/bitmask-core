use anyhow::{Error, Result};
use bdk::{blockchain::Blockchain, database::AnyDatabase, SignOptions, Wallet};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction, Transaction};

use crate::{debug, operations::bitcoin::balance::get_blockchain};

/// Signs and broadcasts a transaction given a Psbt
pub async fn sign_psbt(
    wallet: &Wallet<AnyDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<Transaction> {
    debug!("Signing PSBT...");
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    debug!(format!("Finalized: {finalized}"));
    if finalized {
        debug!("Signed PSBT:", base64::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        debug!("tx:", base64::encode(&serialize(&tx.clone())));
        let blockchain = get_blockchain();
        blockchain.broadcast(&tx).await?;
        Ok(tx)
    } else {
        Err(Error::msg("Could not finalize when signing PSBT"))
    }
}

// Only signs an original psbt.
pub async fn sign_original_psbt(
    wallet: &Wallet<AnyDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<PartiallySignedTransaction> {
    debug!("Funding PSBT...");
    let opts = SignOptions {
        remove_partial_sigs: false,
        ..Default::default()
    };
    wallet.sign(&mut psbt, opts)?;
    Ok(psbt)
}
