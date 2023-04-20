#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::operations::rgb::{
    invoice::pay_invoice,
    psbt::{create_psbt, extract_commit},
};

mod rgb_test_utils;
use rgb_test_utils::{dumb_psbt, generate_new_contract, generate_new_invoice, DumbResolve};
use rgbstd::persistence::Stock;

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
        None,
        &tx_resolver,
    );
    assert!(psbt.is_ok());

    Ok(())
}

#[tokio::test]
async fn allow_extract_commitment() -> anyhow::Result<()> {
    let mut stock = Stock::default();
    let psbt = dumb_psbt();

    let contract_id = generate_new_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = generate_new_invoice(contract_id, seal, &mut stock);

    let result = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(result.is_ok());

    let (psbt, _) = result.unwrap();

    println!("{}", psbt.to_string());

    let commit = extract_commit(psbt);
    assert!(commit.is_ok());
    Ok(())
}
