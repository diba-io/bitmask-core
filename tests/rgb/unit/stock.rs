#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::util::init_logging;
use rgbstd::persistence::{Inventory, Stash, Stock};

use crate::rgb::unit::utils::create_fake_contract;

#[tokio::test]
async fn allow_list_contracts() -> anyhow::Result<()> {
    init_logging("rgb_stock=warn");

    let mut stock = Stock::default();
    create_fake_contract(&mut stock);

    let mut contracts = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            for contract_id in stock.contract_ids().expect("invalid contracts state") {
                if stock.contract_iface(contract_id, iface_id).is_ok() {
                    let iface = stock.iface_by_id(iface_id).expect("");
                    let iface_name = iface.name.to_string();
                    let contract = contract_id.to_string();
                    contracts.push(format!("{iface_name}:{contract}"));
                }
            }
        }
    }
    assert!(!contracts.is_empty());
    Ok(())
}

#[tokio::test]
async fn allow_list_interfaces() -> anyhow::Result<()> {
    init_logging("rgb_stock=warn");

    let mut stock = Stock::default();
    create_fake_contract(&mut stock);

    let mut interfaces = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, iimpl) in schema.clone().iimpls.into_iter() {
            let iface = stock.iface_by_id(iface_id).expect("");
            let iface_name = iface.name.to_string();
            let iimpl_id = iimpl.impl_id().to_string();
            interfaces.push(format!("{iface_name}:{iimpl_id}"));
        }
    }
    assert!(!interfaces.is_empty());
    Ok(())
}

#[tokio::test]
async fn allow_list_schemas() -> anyhow::Result<()> {
    init_logging("rgb_stock=warn");

    let mut stock = Stock::default();
    create_fake_contract(&mut stock);

    let mut schemas = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            let iface = stock.iface_by_id(iface_id).expect("");
            let iface_name = iface.name.to_string();
            schemas.push(format!("{schema_id}:{iface_name}"));
        }
    }
    assert!(!schemas.is_empty());
    Ok(())
}
