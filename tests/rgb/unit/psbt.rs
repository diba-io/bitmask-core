#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::unit::utils::{
    create_fake_contract, create_fake_invoice, create_fake_psbt, DumbResolve,
};
use bitmask_core::{
    rgb::{
        psbt::{create_psbt, extract_commit},
        transfer::pay_invoice,
    },
    structs::{PsbtInputRequest, SecretString},
    util::init_logging,
};
use rgb::persistence::Stock;

#[tokio::test]
async fn allow_create_psbt_file() -> anyhow::Result<()> {
    init_logging("rgb_psbt=warn");

    let desc = "tr(m=[280a5963]/86h/1h/0h=[tpubDCa3US185mM8yGTXtPWY1wNRMCiX89kzN4dwTMKUJyiJnnq486MTeyYShvHiS8Dd1zR2myy5xyJFDs5YacVHn6JZbVaDAtkrXZE3tTVRHPu]/*/*)#8an50cqp";
    let asset_utxo = "5ca6cd1f54c081c8b3a7b4bcc988e55fe3c420ac87512b53a58c55233e15ba4f:1";
    let asset_utxo_terminal = "/0/0";

    let fee = 1000;
    let tx_resolver = DumbResolve {};

    let psbt = create_psbt(
        vec![PsbtInputRequest {
            descriptor: SecretString(desc.to_string()),
            utxo: asset_utxo.to_string(),
            utxo_terminal: asset_utxo_terminal.to_string(),
            tapret: None,
        }],
        vec![],
        fee,
        Some("/0/1".to_string()),
        None,
        &tx_resolver,
    );
    assert!(psbt.is_ok());

    Ok(())
}

#[tokio::test]
async fn allow_extract_commit_from_psbt() -> anyhow::Result<()> {
    let mut stock = Stock::default();
    let psbt = create_fake_psbt();

    let contract_id = create_fake_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = create_fake_invoice(contract_id, seal, &mut stock);

    let result = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(result.is_ok());

    let (psbt, _) = result.unwrap();

    let commit = extract_commit(psbt);
    assert!(commit.is_ok());
    Ok(())
}
