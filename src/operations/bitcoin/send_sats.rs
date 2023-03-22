use anyhow::{anyhow, Result};
use bdk::{database::AnyDatabase, wallet::tx_builder::TxOrdering, FeeRate, Wallet};
use bitcoin::{consensus::serialize, Transaction};
use payjoin::{PjUri, PjUriExt};

use crate::{
    data::structs::SatsInvoice,
    debug, info,
    operations::bitcoin::{
        balance::synchronize_wallet,
        psbt::{sign_original_psbt, sign_psbt},
    },
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

pub async fn create_payjoin(
    invoices: Vec<SatsInvoice>,
    wallet: &Wallet<AnyDatabase>,
    fee_rate: Option<FeeRate>,
    pj_uri: PjUri<'_>, // TODO specify Uri<PayJoinParams>
) -> Result<Transaction> {
    let (psbt, details) = {
        let mut builder = wallet.build_tx();
        for invoice in invoices {
            builder.add_recipient(invoice.address.script_pubkey(), invoice.amount);
        }
        builder.enable_rbf().fee_rate(fee_rate.unwrap_or_default());
        builder.finish()?
    };

    debug!(format!("Request PayJoin transaction: {details:#?}"));
    debug!("Unsigned Original PSBT:", base64::encode(&serialize(&psbt)));
    let original_psbt = sign_original_psbt(wallet, psbt).await?;
    info!("Original PSBT successfully signed");

    // TODO use fee_rate
    let pj_params = payjoin::sender::Configuration::non_incentivizing();
    let (req, ctx) = pj_uri.create_pj_request(original_psbt, pj_params)?;
    info!("Built PayJoin request");
    let response = reqwest::Client::new()
        .post(req.url)
        .header("Content-Type", "text/plain")
        .body(reqwest::Body::from(req.body))
        .send()
        .await?;
    info!("Got PayJoin response");

    let res = response.text().await?;
    info!(format!("Response: {res}"));

    if res.contains("errorCode") {
        return Err(anyhow!("Error performing payjoin: {res}"));
    }

    let payjoin_psbt = ctx.process_response(res.as_bytes())?;

    debug!(
        "Proposed PayJoin PSBT:",
        base64::encode(&serialize(&payjoin_psbt))
    );
    // sign_psbt also broadcasts;
    let tx = sign_psbt(wallet, payjoin_psbt).await?;

    Ok(tx)
}
