use anyhow::Result;
use bitmask_core::operations::lightning::{
    auth, create_invoice, create_wallet, decode_invoice, get_balance, get_txs, pay_invoice, Tx,
};
use log::info;
use regex::Regex;

const INVOICE: &str = "lnbcrt500u1p3n7543pp57ct97x2j26mlq6nhm2zyge2g0wt99wfy3a5p8h5xgkr6yn8xaehsdqqcqzpgxqyz5vqsp5t9tsx0y568dejxdswzjrj5ytmtvr3fvgcl8l7apm9hsxgmsvf20q9qyyssqw84gyjyy5p3gfuhnqnw9gm4j8m3e7gx8phxsumm03jhep2canggj9v7upnqtxzn8dlnw8kfk995l4mlxjnt990axktu4qupn3kkw9sspzd79je";
const TOKEN_ACCOUNT_WITH_BALANCE: &str = "1d61b3aa68a79d62bcd24a0d7e2d00a8544af2db";

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
    let tokens = auth(creds).await?;
    info!("tokens: {tokens:?}");
    let re = Regex::new(r"^[a-f0-9]{40}$").unwrap();
    assert!(re.is_match(&tokens.refresh_token) && re.is_match(&tokens.access_token));

    Ok(())
}

#[tokio::test]
pub async fn decode_invoice_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds).await?;
    let decoded_invoice = decode_invoice(INVOICE, &tokens.access_token).await?;
    info!("decoded_invoice: {decoded_invoice:#?}");
    assert_eq!(decoded_invoice.num_satoshis, "50000");

    Ok(())
}

#[tokio::test]
pub async fn create_invoice_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds).await?;
    let invoice = create_invoice("testing create_invoice", 333, &tokens.access_token).await?;
    let decoded_invoice = decode_invoice(&invoice, &tokens.access_token).await?;
    assert_eq!(decoded_invoice.num_satoshis, "333");
    info!("create_invoice: {invoice}");

    Ok(())
}

#[tokio::test]
pub async fn get_balance_test() -> Result<()> {
    let balance = get_balance(TOKEN_ACCOUNT_WITH_BALANCE).await?;
    assert_eq!(balance.btc.available_balance, 472);
    info!("balance: {balance:#?}");

    Ok(())
}

#[tokio::test]
pub async fn pay_invoice_test() -> Result<()> {
    // We create user Alice
    let alice_creds = create_wallet().await?;
    let alice_tokens = auth(alice_creds).await?;
    // Alice invoice
    let invoice =
        create_invoice("testing pay alice invoice", 33, &alice_tokens.access_token).await?;
    // We create user Bob
    let bob_creds = create_wallet().await?;
    let bob_tokens = auth(bob_creds).await?;
    // We try to pay alice invoice from bob, which have balance = 0
    let response = pay_invoice(&invoice, &bob_tokens.access_token).await?;
    // assert_eq!(balance.btc.available_balance, 838);
    info!("pay_invoice: {response:#?}");

    Ok(())
}

#[tokio::test]
pub async fn get_txs_test() -> Result<()> {
    let txs: Vec<Tx> = get_txs(TOKEN_ACCOUNT_WITH_BALANCE, 0, 0).await?;

    info!("get_txs_test: {txs:#?}");

    Ok(())
}
