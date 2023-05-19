#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::{rgb::issue::issue_contract, util::init_logging};
use rgbstd::persistence::Stock;

use crate::rgb::unit::utils::DumbResolve;

#[tokio::test]
async fn issue_contract_test() -> Result<()> {
    init_logging("rgb_issue=warn");

    let ticker = "DIBA";
    let name = "DIBA";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 10;
    let iface = "RGB20";
    let seal = "tapret1st:70339a6b27f55105da2d050babc759f046c21c26b7b75e9394bc1d818e50ff52:0";
    let network = "regtest";

    let mut stock = Stock::default();
    let mut resolver = DumbResolve {};

    let contract = issue_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        iface,
        seal,
        network,
        None,
        &mut resolver,
        &mut stock,
    );

    assert!(contract.is_ok());
    Ok(())
}
