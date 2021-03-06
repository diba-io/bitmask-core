use anyhow::Result;
use bdk::{database::MemoryDatabase, FeeRate, TransactionDetails, Wallet};
use bitcoin::consensus::serialize;

use crate::{
    data::structs::SatsInvoice,
    log,
    operations::bitcoin::{balance::synchronize_wallet, sign_psbt::sign_psbt},
};

pub async fn create_transaction(
    invoices: Vec<SatsInvoice>,
    wallet: &Wallet<MemoryDatabase>,
) -> Result<TransactionDetails> {
    synchronize_wallet(wallet).await?;
    let (psbt, details) = {
        let mut builder = wallet.build_tx();
        for invoice in invoices {
            builder.add_recipient(invoice.address.script_pubkey(), invoice.amount);
        }
        builder.enable_rbf().fee_rate(FeeRate::from_sat_per_vb(1.0));
        builder.finish()?
    };

    log!(format!("Transaction details: {details:#?}"));
    log!("Unsigned PSBT: {}", base64::encode(&serialize(&psbt)));
    sign_psbt(wallet, psbt).await?;
    log!("PSBT successfully signed");

    Ok(details)
}
