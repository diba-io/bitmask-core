#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::structs::MediaInfo;

use crate::rgb::integration::utils::issuer_issue_contract;

#[tokio::test]
async fn allow_issuer_issue_fungible_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB20", 5, false, true, None).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_issue_uda_contract() -> anyhow::Result<()> {
    let media_info = Some(vec![MediaInfo {
        ty: "image/png".to_string(),
        source: "https://carbonado.io/diba.png".to_string(),
    }]);
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, true, media_info).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}
