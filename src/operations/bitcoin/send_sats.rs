use std::str::FromStr;

use anyhow::Result;
use bdk::{database::MemoryDatabase, FeeRate, Wallet};
use bitcoin::{consensus::serialize, util::address::Address};
use gloo_console::log;

use crate::{
    data::structs::SatsInvoice,
    operations::bitcoin::{balance::synchronize_wallet, sign_psbt::sign_psbt},
};

pub async fn create_transaction(
    invoices: Vec<SatsInvoice>,
    wallet: &Wallet<MemoryDatabase>,
) -> Result<String> {
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
    let signing = sign_psbt(wallet, psbt).await;
    match signing {
        Ok(_signing) => Ok(serde_json::to_string(&details)?),
        Err(_e) => Ok("Server error".to_string()),
    }
}
