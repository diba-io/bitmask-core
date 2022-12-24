use anyhow::Result;
use bdk::{database::AnyDatabase, wallet::tx_builder::TxOrdering, FeeRate, Wallet};
use bitcoin::{consensus::serialize, Transaction};

use crate::{
    data::structs::SatsInvoice,
    debug, info,
    operations::bitcoin::{balance::synchronize_wallet, psbt::sign_psbt},
};

pub async fn create_transaction(
    invoices: Vec<SatsInvoice>,
    wallet: &Wallet<AnyDatabase>,
    fee_rate: Option<FeeRate>,
) -> Result<Transaction> {
    synchronize_wallet(wallet).await?;
    let (psbt, details) = {
        let mut builder = wallet.build_tx();
        for invoice in invoices {
            builder.add_recipient(invoice.address.script_pubkey(), invoice.amount);
        }
        builder.ordering(TxOrdering::Untouched); // TODO: Remove after implementing wallet persistence
        builder.enable_rbf().fee_rate(fee_rate.unwrap_or_default());
        builder.finish()?
    };

    debug!(format!("Create transaction: {details:#?}"));
    debug!("Unsigned PSBT:", base64::encode(&serialize(&psbt)));
    let tx = sign_psbt(wallet, psbt).await?;
    info!("PSBT successfully signed");

    Ok(tx)
}
