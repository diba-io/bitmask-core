#![cfg(not(target_arch = "wasm32"))]
use amplify::hex::ToHex;
use bitmask_core::{
    rgb::transfer::{accept_transfer, create_invoice, pay_invoice},
    util::init_logging,
};
use rgbstd::persistence::Stock;
use strict_encoding::StrictSerialize;

use crate::rgb::unit::utils::{
    create_fake_contract, create_fake_invoice, create_fake_psbt, DumbResolve,
};

#[tokio::test]
async fn allow_create_invoice() -> anyhow::Result<()> {
    init_logging("rgb_invoice=warn");

    let iface = "RGB20";
    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let amount = 1;

    let mut stock = Stock::default();
    let contract_id = create_fake_contract(&mut stock);
    let result = create_invoice(&contract_id.to_string(), iface, amount, seal, &mut stock);

    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_pay_invoice() -> anyhow::Result<()> {
    init_logging("rgb_invoice=warn");

    let mut resolver = DumbResolve {};
    let mut stock = Stock::default();
    let psbt = create_fake_psbt();

    let contract_id = create_fake_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = create_fake_invoice(contract_id, seal, &mut stock);

    let result = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(result.is_ok());

    let (_, transfer) = result.unwrap();

    let pay_status = transfer.unbindle().validate(&mut resolver);
    assert!(pay_status.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_accept_invoice() -> anyhow::Result<()> {
    init_logging("rgb_invoice=warn");

    let mut resolver = DumbResolve {};
    let mut stock = Stock::default();
    let psbt = create_fake_psbt();

    let contract_id = create_fake_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = create_fake_invoice(contract_id, seal, &mut stock);

    let result = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(result.is_ok());

    let (_, transfer) = result.unwrap();

    let transfer_hex = transfer
        .to_strict_serialized::<0xFFFFFF>()
        .unwrap()
        .to_hex();

    let pay_status = accept_transfer(transfer_hex, true, &mut resolver, &mut stock);
    assert!(pay_status.is_ok());
    Ok(())
}
