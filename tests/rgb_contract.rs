#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::operations::rgb::{
    issue_contract::issue_contract, shared::default_fungible_iimpl,
};
use rgbstd::interface::rgb20;

#[tokio::test]
async fn issue_contract_test() -> Result<()> {
    let ticker = "DIBA1";
    let name = "DIBA1";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 1;
    let seal = "tapret1st:c6dd3ff7130c0a15cfdad166ff46fb3f71485f5451e21509027a037298ba1a3b:1";

    let iface = rgb20();
    let iimpl = default_fungible_iimpl();

    let contract = issue_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        seal,
        iface,
        iimpl,
    );

    assert!(contract.is_ok());

    Ok(())
}
