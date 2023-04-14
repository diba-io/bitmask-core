#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::operations::rgb::pay::pay_asset;
mod rgb_test_utils;
use rgb_test_utils::{dumb_contract, dumb_invoice, dumb_psbt, DumbResolve};
use rgbstd::persistence::Stock;

#[tokio::test]
async fn allow_pay_and_accept_payment() -> anyhow::Result<()> {
    let (contract_id, stock) = dumb_contract(Stock::default());
    let psbt = dumb_psbt();

    let vout = 1;
    let txid = "ced67bf611741dd5b2f749fdd37d33abb688c1a66a7d8d9ed8c3d89d9d59eba7".to_string();
    let invoice = dumb_invoice(contract_id, stock.clone(), txid, vout);

    let transfer = pay_asset(invoice, psbt, stock);
    assert!(transfer.is_ok());

    let mut resolver = DumbResolve {};
    let _status = transfer?.unbindle().validate(&mut resolver);
    // let status = accept_transfer::<DumbResolve>(status, stock, &mut resolver).expect("");

    Ok(())
}
