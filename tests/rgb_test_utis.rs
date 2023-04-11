use std::convert::Infallible;

use bitmask_core::{
    operations::rgb::issue_contract::issue_contract,
    operations::rgb::schemas::default_fungible_iimpl,
};
use bp::Txid;
use rgbstd::{
    containers::BindleContent,
    contract::ContractId,
    interface::rgb20,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::ResolveTx,
};

pub struct DumbResolve {}

impl ResolveHeight for DumbResolve {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> std::result::Result<u32, Self::Error> {
        Ok(0)
    }
}

impl ResolveTx for DumbResolve {
    fn resolve_tx(&self, _txid: Txid) -> Result<bp::Tx, rgbstd::validation::TxResolverError> {
        todo!()
    }
}

pub fn build_contract(mut stock: Stock) -> (ContractId, Stock) {
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
    )
    .expect("");

    let mut dumb = DumbResolve {};

    let bindle = contract.bindle();
    let contract = bindle
        .unbindle()
        .validate(&mut dumb)
        .map_err(|c| c.validation_status().expect("just validated").to_string())
        .expect("");

    stock
        .import_contract(contract.clone(), &mut dumb)
        .expect("");

    (contract.contract_id().clone(), stock)
}
