use anyhow::{Error, Result};
use bdk::{blockchain::Blockchain, database::MemoryDatabase, SignOptions, Wallet};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction, Transaction};

use crate::{debug, operations::bitcoin::balance::get_blockchain};

pub async fn sign_psbt(
    wallet: &Wallet<MemoryDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<Transaction> {
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    if finalized {
        debug!("Signed PSBT:", hex::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        debug!("tx:", hex::encode(&serialize(&tx)));
        let blockchain = get_blockchain();
        blockchain.broadcast(&tx).await?;
        Ok(tx)
    } else {
        Err(Error::msg(""))
    }
}
