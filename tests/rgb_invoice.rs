#![cfg(not(target_arch = "wasm32"))]
use amplify::hex::ToHex;
use bitmask_core::{
    operations::rgb::invoice::{accept_payment, create_invoice, pay_invoice},
    util::init_logging,
};
use rgbstd::persistence::Stock;

mod rgb_test_utils;
use rgb_test_utils::generate_new_contract;
use strict_encoding::StrictSerialize;

use crate::rgb_test_utils::{dumb_psbt, generate_new_invoice, DumbResolve};

#[tokio::test]
async fn allow_create_invoice() -> anyhow::Result<()> {
    init_logging("rgb_invoice=warn");

    let iface = "RGB20";
    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let amount = 1;

    let mut stock = Stock::default();
    let contract_id = generate_new_contract(&mut stock);
    let invoice = create_invoice(&contract_id.to_string(), iface, amount, seal, &mut stock);

    assert!(invoice.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_pay_invoice() -> anyhow::Result<()> {
    init_logging("rgb_invoice=warn");

    let mut resolver = DumbResolve {};
    let mut stock = Stock::default();
    let psbt = dumb_psbt();

    let contract_id = generate_new_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = generate_new_invoice(contract_id, seal, &mut stock);

    let transfer = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(transfer.is_ok());

    let pay_status = transfer?.unbindle().validate(&mut resolver);
    assert!(pay_status.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_accept_invoice() -> anyhow::Result<()> {
    init_logging("rgb_invoice=warn");

    let mut resolver = DumbResolve {};
    let mut stock = Stock::default();
    let psbt = dumb_psbt();

    let contract_id = generate_new_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = generate_new_invoice(contract_id, seal, &mut stock);

    let transfer = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(transfer.is_ok());

    let transfer_hex = transfer
        .unwrap()
        .to_strict_serialized::<0xFFFFFF>()
        .unwrap()
        .to_hex();

    let pay_status = accept_payment(transfer_hex, true, &mut resolver, &mut stock);
    assert!(pay_status.is_ok());
    Ok(())
}
