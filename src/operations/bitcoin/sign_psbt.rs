use anyhow::{Error, Result};
use bdk::{blockchain::Blockchain, database::MemoryDatabase, SignOptions, Wallet};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction, Transaction};

use crate::{log, operations::bitcoin::balance::get_blockchain};

pub async fn sign_psbt(
    wallet: &Wallet<MemoryDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<Transaction> {
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    if finalized {
        log!("Signed PSBT: {}", base64::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        log!("tx: {}", base64::encode(&serialize(&tx)));
        let blockchain = get_blockchain();
        blockchain.broadcast(&tx).await?;
        Ok(tx)
    } else {
        Err(Error::msg(""))
    }
}
