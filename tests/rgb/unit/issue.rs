#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::{
    rgb::issue::issue_contract, structs::IssueRequest, util::init_logging, validators::RGBContext,
};
use garde::Validate;
use rgbstd::persistence::Stock;

use crate::rgb::unit::utils::{get_uda_data, DumbResolve};

#[tokio::test]
async fn issue_request_params_check() -> Result<()> {
    init_logging("rgb_issue=warn");

    let ticker = "DIBA";
    let name = "DIBA";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 10;
    let iface = "RGB20";
    let seal = "tapret1st:70339a6b27f55105da2d050babc759f046c21c26b7b75e9394bc1d818e50ff52:0";

    let ctx = &RGBContext::default();
    let rgb20 = IssueRequest {
        ticker: ticker.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        supply,
        precision,
        seal: seal.to_string(),
        iface: iface.to_string(),
        meta: None,
    };
    assert!(rgb20.validate(ctx).is_ok());

    let rgb21 = IssueRequest {
        ticker: ticker.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        supply,
        precision,
        seal: seal.to_string(),
        iface: iface.to_string(),
        meta: Some(get_uda_data()),
    };
    assert!(rgb21.validate(ctx).is_ok());

    Ok(())
}

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
