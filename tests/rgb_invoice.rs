#![cfg(not(target_arch = "wasm32"))]
use std::str::FromStr;

use bitmask_core::operations::rgb::invoice::{create_invoice, InvoiceError};
use bp::Txid;
use rgbstd::{contract::GraphSeal, interface::rgb20, persistence::Stock};

mod rgb_test_utis;
use rgb_test_utis::dumb_contract;

#[tokio::test]
async fn allow_create_simple_contract_test() -> anyhow::Result<()> {
    let iface = rgb20();
    let amount = 1;
    let txid =
        Txid::from_str("ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e").unwrap();

    let (contract_id, stock) = dumb_contract(Stock::default());

    let seal = GraphSeal::tapret_first(txid, 0);
    let invoice = create_invoice(contract_id, iface, amount, seal, stock);

    assert!(invoice.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_create_only_tapret_first_seals() -> anyhow::Result<()> {
    let iface = rgb20();
    let amount = 1;
    let txid =
        Txid::from_str("ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e").unwrap();

    let (contract_id, stock) = dumb_contract(Stock::default());

    let seal = GraphSeal::opret_first(txid, 0);
    let invoice = create_invoice(contract_id, iface, amount, seal, stock);

    assert!(invoice.is_err());
    assert_eq!(InvoiceError::InvalidBlindSeal, invoice.err().unwrap());
    Ok(())
}

#[tokio::test]
async fn allow_only_known_contracts() -> anyhow::Result<()> {
    let iface = rgb20();
    let amount = 1;
    let txid =
        Txid::from_str("ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e").unwrap();

    let (contract_id, _) = dumb_contract(Stock::default());
    let empty_stock = Stock::default();

    let seal = GraphSeal::tapret_first(txid, 0);
    let invoice = create_invoice(contract_id, iface, amount, seal, empty_stock);

    assert!(invoice.is_err());
    assert_eq!(
        InvoiceError::ContractNotfound(contract_id),
        invoice.err().unwrap()
    );
    Ok(())
}
