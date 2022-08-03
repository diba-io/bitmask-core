use anyhow::Result;
use bdk::{database::AnyDatabase, FeeRate, Wallet};
use bitcoin::{consensus::serialize, Transaction};

use crate::{
    data::structs::SatsInvoice,
    debug, info,
    operations::bitcoin::{balance::synchronize_wallet, sign_psbt::sign_psbt},
};

pub async fn create_transaction(
    invoices: Vec<SatsInvoice>,
    wallet: &Wallet<AnyDatabase>,
) -> Result<Transaction> {
    synchronize_wallet(wallet).await?;
    let (psbt, details) = {
        let mut builder = wallet.build_tx();
        for invoice in invoices {
            builder.add_recipient(invoice.address.script_pubkey(), invoice.amount);
        }
        builder.enable_rbf().fee_rate(FeeRate::from_sat_per_vb(1.0));
        builder.finish()?
    };

    debug!(format!("Create transaction: {details:#?}"));
    debug!("Unsigned PSBT:", base64::encode(&serialize(&psbt)));
    let tx = sign_psbt(wallet, psbt).await?;
    info!("PSBT successfully signed");

    Ok(tx)
}
