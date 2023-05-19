#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::issuer_issue_contract;

#[tokio::test]
async fn allow_issuer_issue_fungible_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB20", 5, false).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_issue_uda_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB21", 1, false).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}
