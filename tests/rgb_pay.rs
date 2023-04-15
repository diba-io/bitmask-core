#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::operations::rgb::pay::pay_asset;
mod rgb_test_utils;
use rgb_test_utils::{dumb_psbt, generate_new_contract, generate_new_invoice, DumbResolve};
use rgbstd::{
    persistence::{Inventory, Stock},
    validation::Validity,
};

#[tokio::test]
async fn allow_pay_and_accept_payment() -> anyhow::Result<()> {
    let (contract_id, mut stock) = generate_new_contract(Stock::default());
    let psbt = dumb_psbt();

    let txid = "5ca6cd1f54c081c8b3a7b4bcc988e55fe3c420ac87512b53a58c55233e15ba4f";
    let vout = 1;
    let invoice = generate_new_invoice(contract_id, stock.clone(), txid.to_string(), vout);

    let transfer = pay_asset(invoice, psbt, stock.clone());
    assert!(transfer.is_ok());

    let mut resolver = DumbResolve {};
    let pay_status = transfer?.unbindle().validate(&mut resolver);
    assert!(pay_status.is_ok());

    let transfer = pay_status.expect("fail");
    let accept_status = stock.accept_transfer(transfer, &mut resolver, true);
    assert!(accept_status.is_ok());
    assert_eq!(accept_status.unwrap().validity(), Validity::Valid);
    Ok(())
}
