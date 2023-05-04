#![cfg(not(target_arch = "wasm32"))]
#[tokio::test]
/*
 * Issuer to Beneficiary
 */
async fn allow_issuer_issue_contract() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_beneficiary_create_invoice() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_issuer_create_psbt() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_issuer_transfer_asset() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_beneficiary_accept_tranfer() -> anyhow::Result<()> {
    Ok(())
}
