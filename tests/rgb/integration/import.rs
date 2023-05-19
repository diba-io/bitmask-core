#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{import_new_contract, issuer_issue_contract};

#[tokio::test]
async fn allow_import_fungible_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB20", false).await;
    assert!(issuer_resp.is_ok());

    let import_resp = import_new_contract(issuer_resp?).await;
    assert!(import_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_import_uda_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB21", false).await;
    assert!(issuer_resp.is_ok());

    let import_resp = import_new_contract(issuer_resp?).await;
    assert!(import_resp.is_ok());
    Ok(())
}
