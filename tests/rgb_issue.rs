#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::operations::rgb::{issue::issue_contract, schemas::default_fungible_iimpl};
use rgbstd::interface::rgb20;

#[tokio::test]
async fn issue_contract_test() -> Result<()> {
    let ticker = "DIBA1";
    let name = "DIBA1";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 10;
    let seal = "tapret1st:70339a6b27f55105da2d050babc759f046c21c26b7b75e9394bc1d818e50ff52:0";

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
