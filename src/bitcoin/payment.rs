use anyhow::{anyhow, Result};

use bdk::{
    database::AnyDatabase, wallet::tx_builder::TxOrdering, FeeRate, TransactionDetails, Wallet,
};

use bitcoin::consensus::{
    consensus::serialize,
    psbt::{Input, Psbt},
    TxIn,
};

use payjoin::{PjUri, PjUriExt};

use crate::{
    bitcoin::{
        psbt::{sign_original_psbt, sign_psbt},
        wallet::MemoryWallet,
    },
    debug, info,
    structs::SatsInvoice,
};

pub async fn create_transaction(
    invoices: Vec<SatsInvoice>,
    wallet: &MemoryWallet,
    fee_rate: Option<FeeRate>,
) -> Result<TransactionDetails> {
    let (psbt, details) = {
        let locked_wallet = wallet.lock().await;
        let mut builder = locked_wallet.build_tx();
        for invoice in invoices {
            builder.add_recipient(invoice.address.script_pubkey(), invoice.amount);
        }
        builder.ordering(TxOrdering::Untouched); // TODO: Remove after implementing wallet persistence
        builder.enable_rbf().fee_rate(fee_rate.unwrap_or_default());
        builder.finish()?
    };

    debug!(format!("Create transaction: {details:#?}"));
    debug!("Unsigned PSBT:", base64::encode(&serialize(&psbt)));
    let details = sign_psbt(wallet, psbt).await?;
    info!("PSBT successfully signed");

    Ok(details)
}

pub async fn create_payjoin(
    invoices: Vec<SatsInvoice>,
    wallet: &MemoryWallet,
    fee_rate: Option<FeeRate>,
    pj_uri: PjUri<'_>, // TODO specify Uri<PayJoinParams>
) -> Result<TransactionDetails> {
    let (psbt, details) = {
        let locked_wallet = wallet.lock().await;
        let mut builder = locked_wallet.build_tx();
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
    let (req, ctx) = pj_uri.create_pj_request(original_psbt.clone(), pj_params)?;
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
    let payjoin_psbt = add_back_original_input(&original_psbt, payjoin_psbt);

    debug!(
        "Proposed PayJoin PSBT:",
        base64::encode(&serialize(&payjoin_psbt))
    );
    // sign_psbt also broadcasts;
    let tx = sign_psbt(wallet, payjoin_psbt).await?;

    Ok(tx)
}

/// Unlike Bitcoin Core's walletprocesspsbt RPC, BDK's finalize_psbt only checks
/// if the script in the PSBT input map matches the descriptor and does not
/// check whether it has control of the OutPoint specified in the unsigned_tx's
/// TxIn. So the original_psbt input data needs to be added back into
/// payjoin_psbt without overwriting receiver input.
fn add_back_original_input(original_psbt: &Psbt, payjoin_psbt: Psbt) -> Psbt {
    // input_pairs is only used here. It may be added to payjoin, rust-bitcoin, or BDK in time.
    fn input_pairs(psbt: &Psbt) -> Box<dyn Iterator<Item = (TxIn, Input)> + '_> {
        Box::new(
            psbt.unsigned_tx
                .input
                .iter()
                .cloned() // Clone each TxIn for better ergonomics than &muts
                .zip(psbt.inputs.iter().cloned()), // Clone each Input too
        )
    }

    let mut original_inputs = input_pairs(&original_psbt).peekable();

    for (proposed_txin, mut proposed_psbtin) in input_pairs(&payjoin_psbt) {
        if let Some((original_txin, original_psbtin)) = original_inputs.peek() {
            if proposed_txin.previous_output == original_txin.previous_output {
                proposed_psbtin.witness_utxo = original_psbtin.witness_utxo.clone();
                proposed_psbtin.non_witness_utxo = original_psbtin.non_witness_utxo.clone();
            }
            original_inputs.next();
        }
    }
    payjoin_psbt
}
