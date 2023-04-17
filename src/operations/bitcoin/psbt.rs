use anyhow::{Error, Result};
use bdk::{
    blockchain::Blockchain, database::AnyDatabase, psbt::PsbtUtils, SignOptions,
    TransactionDetails, Wallet,
};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction};

use crate::{debug, operations::bitcoin::balance::get_blockchain, synchronize_wallet};

/// Signs and broadcasts a transaction given a Psbt
pub async fn sign_psbt(
    wallet: &Wallet<AnyDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<TransactionDetails> {
    debug!("Signing PSBT...");
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    debug!(format!("Finalized: {finalized}"));
    if finalized {
        debug!("Signed PSBT:", base64::encode(&serialize(&psbt)));
        let fee_amount = psbt.fee_amount().expect("fee amount on PSBT is known");
        let tx = psbt.extract_tx();
        debug!("tx:", base64::encode(&serialize(&tx.clone())));
        let blockchain = get_blockchain();
        blockchain.broadcast(&tx).await?;
        synchronize_wallet(wallet).await?;
        let txid = tx.txid();
        let tx = blockchain
            .get_tx(&txid)
            .await?
            .expect("tx that was just broadcasted now exists");

        let sent = tx.output.iter().fold(0, |sum, output| output.value + sum);

        let details = TransactionDetails {
            transaction: Some(tx),
            txid,
            received: sent - fee_amount,
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
