use anyhow::{Error, Result};
use bdk::{blockchain::Blockchain, psbt::PsbtUtils, SignOptions, TransactionDetails};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction};

use crate::{
    bitcoin::{get_blockchain, MemoryWallet},
    debug,
};

/// Signs and broadcasts a transaction given a Psbt
pub async fn sign_psbt(
    wallet: &MemoryWallet,
    mut psbt: PartiallySignedTransaction,
) -> Result<TransactionDetails> {
    debug!("Signing PSBT...");
    let finalized = wallet
        .lock()
        .await
        .sign(&mut psbt, SignOptions::default())?;
    debug!(format!("Finalized: {finalized}"));
    if finalized {
        debug!("Signed PSBT:", base64::encode(&serialize(&psbt)));
        let fee_amount = psbt.fee_amount().expect("fee amount on PSBT is known");
        let tx = psbt.extract_tx();
        debug!("tx:", base64::encode(&serialize(&tx.clone())));
        let blockchain = get_blockchain().await;
        blockchain.broadcast(&tx).await?;

        let txid = tx.txid();
        let tx = blockchain
            .get_tx(&txid)
            .await
            .expect("tx that was just broadcasted now exists");

        let mut sent = 0;
        let mut received = 0;

        if let Some(tx) = tx.clone() {
            sent = tx.output.iter().fold(0, |sum, output| output.value + sum);
            received = sent - fee_amount;
        }

        let details = TransactionDetails {
            transaction: tx,
            txid,
            received,
            sent,
            fee: Some(fee_amount),
            confirmation_time: None,
        };

        Ok(details)
    } else {
        Err(Error::msg("Could not finalize when signing PSBT"))
    }
}

// Only signs an original psbt.
pub async fn sign_original_psbt(
    wallet: &MemoryWallet,
    mut psbt: PartiallySignedTransaction,
) -> Result<PartiallySignedTransaction> {
    debug!("Funding PSBT...");
    let opts = SignOptions {
        remove_partial_sigs: false,
        ..Default::default()
    };
    wallet.lock().await.sign(&mut psbt, opts)?;
    Ok(psbt)
}
