#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::structs::{IssueMetaRequest, IssueMetadata, MediaInfo, NewCollectible};

use crate::rgb::integration::utils::issuer_issue_contract;

#[tokio::test]
async fn allow_issuer_issue_fungible_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB20", 5, false, None).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_issue_uda_contract() -> anyhow::Result<()> {
    let single = Some(IssueMetaRequest::with(vec![IssueMetadata::UDA(
        MediaInfo {
            ty: "image/png".to_string(),
            source: "https://carbonado.io/diba.png".to_string(),
        },
    )]));
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, single).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_issue_collectible_contract() -> anyhow::Result<()> {
    let collectible = Some(IssueMetaRequest::with(vec![IssueMetadata::Collectible(
        vec![
            NewCollectible {
                ticker: "DIBAA".to_string(),
                name: "DIBAA".to_string(),
                preview: MediaInfo {
                    ty: "image/png".to_string(),
                    source: "https://carbonado.io/diba1.png".to_string(),
                },
                ..Default::default()
            },
            NewCollectible {
                ticker: "DIBAB".to_string(),
                name: "DIBAB".to_string(),
                preview: MediaInfo {
                    ty: "image/png".to_string(),
                    source: "https://carbonado.io/diba2.png".to_string(),
                },
                ..Default::default()
            },
        ],
    )]));
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, collectible).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}
