use std::str::FromStr;

use amplify::hex::ToHex;
use bech32::{encode, ToBase32};
use rgb::{Resolver, RgbWallet};
use rgbstd::{
    contract::ContractId,
    interface::{IfaceId, IfacePair},
    persistence::{Inventory, Stock},
};
use strict_encoding::{tn, StrictSerialize, TypeName};

use crate::structs::{ContractFormats, ImportResponse};

use super::wallet::list_allocations;

// TODO: Create one extractor by contract interface
pub fn extract_contract_by_id(
    contract_id: ContractId,
    stock: &mut Stock,
    resolver: &mut impl Resolver,
    wallet: Option<RgbWallet>,
) -> Result<ImportResponse, anyhow::Error> {
    let contract_bindle = stock
        .export_contract(contract_id)
        .expect("contract not found");

    let ifaces: Vec<String> = contract_bindle
        .ifaces
        .keys()
        .map(|f| f.to_string())
        .collect();
    let iface_id = IfaceId::from_str(&ifaces[0]).expect("iface parse error");

    let IfacePair { iface, iimpl } = &contract_bindle
        .ifaces
        .get(&iface_id)
        .expect("contract cannot implemented the iface");

    let iimpl_id = iimpl.impl_id().to_string();
    let contract_id = contract_bindle.contract_id().to_string();

    let contract_legacy = encode(
        "rgb",
        contract_bindle
            .to_strict_serialized::<0xFFFFFF>()
            .expect("invalid contract data")
            .to_base32(),
        bech32::Variant::Bech32m,
    )
    .expect("invalid contract data");
    let contract_strict = contract_bindle
        .to_strict_serialized::<0xFFFFFF>()
        .expect("invalid contract data")
        .to_hex();

    let contract_iface = stock
        .contract_iface(contract_bindle.contract_id(), iface_id.to_owned())
        .expect("invalid contracts state");
    let ty: TypeName = tn!("Ticker");
    let ticker = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let ty: TypeName = tn!("Name");
    let name = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let ty: TypeName = tn!("Details");
    let description = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let ty: TypeName = tn!("Precision");
    let precision = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let mut supply = 0;
    for owned in &contract_iface.iface.assignments {
        if let Ok(allocations) = contract_iface.fungible(owned.name.clone()) {
            for allocation in allocations {
                supply = allocation.value;
            }
        }
    }
    let mut balance = 0;
    let mut allocations = vec![];
    if let Some(wallet) = wallet {
        let mut fetch_wallet = wallet;
        let watcher = list_allocations(&mut fetch_wallet, stock, resolver)
            .expect("invalid allocation states");
        if let Some(watcher_detail) = watcher.into_iter().find(|w| w.contract_id == contract_id) {
            allocations.extend(watcher_detail.allocations);
        }

        balance = allocations
            .clone()
            .into_iter()
            .filter(|a| a.is_mine)
            .map(|a| a.value)
            .sum();
    }

    let resp = ImportResponse {
        contract_id,
        iimpl_id,
        iface: iface.name.to_string(),
        ticker,
        name,
        description,
        precision,
        supply,
        balance,
        allocations,
        contract: ContractFormats {
            legacy: contract_legacy,
            strict: contract_strict,
            armored: contract_bindle.to_string(),
        },
    };

    Ok(resp)
}
