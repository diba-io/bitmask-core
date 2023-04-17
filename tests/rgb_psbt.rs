#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::operations::rgb::psbt::create_psbt;

mod rgb_test_utils;
use rgb_test_utils::DumbResolve;

#[tokio::test]
async fn allow_create_psbt_file() -> anyhow::Result<()> {
    let desc = "tr(m=[280a5963]/86h/1h/0h=[tpubDCa3US185mM8yGTXtPWY1wNRMCiX89kzN4dwTMKUJyiJnnq486MTeyYShvHiS8Dd1zR2myy5xyJFDs5YacVHn6JZbVaDAtkrXZE3tTVRHPu]/*/*)#8an50cqp";
    let asset_utxo = "5ca6cd1f54c081c8b3a7b4bcc988e55fe3c420ac87512b53a58c55233e15ba4f:1";
    let asset_utxo_terminal = "/0/0";

    let fee = 1000;
    let tx_resolver = DumbResolve {};

    let psbt = create_psbt(
        desc.to_string(),
        asset_utxo.to_string(),
        asset_utxo_terminal.to_string(),
        Some("0".to_string()),
        vec![],
        fee,
        &tx_resolver,
    );
    assert!(psbt.is_ok());

    Ok(())
}
