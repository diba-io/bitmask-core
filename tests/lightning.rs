use anyhow::Result;
use bitmask_core::operations::lightning::{
    auth, create_invoice, create_wallet, decode_invoice, get_balance, get_txs, pay_invoice,
    PayInvoiceMessage, Tx,
};
use log::info;
use regex::Regex;

#[tokio::test]
pub async fn create_wallet_test() -> Result<()> {
    pretty_env_logger::init();
    let creds = create_wallet().await?;
    let re = Regex::new(r"^[a-f0-9]{20}$").unwrap();
    assert!(re.is_match(&creds.login) && re.is_match(&creds.password));

    Ok(())
}

#[tokio::test]
pub async fn auth_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds.login, creds.password).await?;
    info!("tokens: {tokens:?}");
    let re = Regex::new(r"^[a-f0-9]{40}$").unwrap();
    assert!(re.is_match(&tokens.refresh_token) && re.is_match(&tokens.access_token));

    Ok(())
}

#[tokio::test]
pub async fn create_decode_invoice_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds.login, creds.password).await?;
    let invoice = create_invoice(
        "testing create_invoice".to_string(),
        333,
        tokens.access_token.clone(),
    )
    .await?;
    let decoded_invoice = decode_invoice(invoice, tokens.access_token).await?;
    info!("decoded_invoice: {decoded_invoice:#?}");
    assert_eq!(decoded_invoice.num_satoshis, "333");

    Ok(())
}

#[tokio::test]
pub async fn get_balance_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds.login, creds.password).await?;
    let balance = get_balance(tokens.access_token).await?;
    assert_eq!(balance.btc.available_balance, 0);
    info!("balance: {balance:#?}");

    Ok(())
}

#[tokio::test]
pub async fn pay_invoice_error_test() -> Result<()> {
    // We create user Alice
    let alice_creds = create_wallet().await?;
    let alice_tokens = auth(alice_creds.login, alice_creds.password).await?;
    // Alice invoice
    let invoice = create_invoice(
        "testing pay alice invoice".to_string(),
        33,
        alice_tokens.access_token,
    )
    .await?;
    // We create user Bob
    let bob_creds = create_wallet().await?;
    let bob_tokens = auth(bob_creds.login, bob_creds.password).await?;
    // We try to pay alice invoice from bob, which have balance = 0
    let response = pay_invoice(invoice, bob_tokens.access_token).await?;
    assert_eq!(
        response,
        PayInvoiceMessage::PayInvoiceError {
            error: true,
            code: 2,
            message:
                "not enough balance. Make sure you have at least 1% reserved for potential fees"
                    .to_string(),
        }
    );
    info!("pay_invoice: {response:#?}");

    Ok(())
}

#[tokio::test]
pub async fn get_txs_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds.login, creds.password).await?;
    let txs: Vec<Tx> = get_txs(tokens.access_token, 0, 0).await?;
    assert_eq!(txs.len(), 0);

    info!("get_txs_test: {txs:#?}");

    Ok(())
}
