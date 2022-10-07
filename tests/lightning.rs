use anyhow::Result;
use bitmask_core::operations::lightning::{auth, create_wallet, decode_invoice};
use log::info;
use regex::Regex;

const INVOICE: &str = "lnbcrt500u1p3n7543pp57ct97x2j26mlq6nhm2zyge2g0wt99wfy3a5p8h5xgkr6yn8xaehsdqqcqzpgxqyz5vqsp5t9tsx0y568dejxdswzjrj5ytmtvr3fvgcl8l7apm9hsxgmsvf20q9qyyssqw84gyjyy5p3gfuhnqnw9gm4j8m3e7gx8phxsumm03jhep2canggj9v7upnqtxzn8dlnw8kfk995l4mlxjnt990axktu4qupn3kkw9sspzd79je";

#[tokio::test]
pub async fn create_wallet_test() -> Result<()> {
    pretty_env_logger::init();
    info!("Creating wallet");
    let creds = create_wallet().await?;
    let re = Regex::new(r"^[a-f0-9]{20}$").unwrap();
    assert!(re.is_match(&creds.login) && re.is_match(&creds.password));

    Ok(())
}

#[tokio::test]
pub async fn auth_test() -> Result<()> {
    let creds = create_wallet().await?;
    info!("creds: {creds:?}");
    let tokens = auth(creds).await?;
    info!("tokens: {tokens:?}");

    Ok(())
}

#[tokio::test]
pub async fn decode_invoice_test() -> Result<()> {
    let creds = create_wallet().await?;
    let tokens = auth(creds).await?;
    let decoded_invoice = decode_invoice(INVOICE, &tokens.access_token).await?;
    info!("decoded_invoice: {decoded_invoice:?}");

    Ok(())
}
