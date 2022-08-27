use anyhow::{Error, Result};
use bdk::{blockchain::Blockchain, database::AnyDatabase, SignOptions, Wallet};
use bitcoin::{consensus::serialize, util::psbt::PartiallySignedTransaction, Transaction};

use crate::{debug, operations::bitcoin::balance::get_blockchain};

pub async fn sign_psbt(
    wallet: &Wallet<AnyDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<Transaction> {
    debug!("Signing PSBT...");
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    if finalized {
        debug!("Signed PSBT:", base64::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        debug!("tx:", base64::encode(&serialize(&tx)));
        let blockchain = get_blockchain();
        blockchain.broadcast(&tx).await?;
        Ok(tx)
    } else {
        Err(Error::msg(""))
    }
}
