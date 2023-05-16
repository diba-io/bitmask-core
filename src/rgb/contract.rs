use std::str::FromStr;

use amplify::{confinement::Confined, hex::ToHex};
use bech32::{encode, ToBase32};
use rgb::{Resolver, RgbWallet};
use rgbstd::{
    contract::ContractId,
    interface::{IfaceId, IfacePair},
    persistence::{Inventory, Stock},
};
use strict_encoding::{tn, FieldName, StrictSerialize, TypeName};
use strict_types::StrictVal;

use crate::structs::{ContractFormats, ImportResponse};
use crate::{rgb::wallet::list_allocations, structs::GenesisFormats};

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

    let mut ticker = String::new();
    let mut name = String::new();
    let mut precision = String::new();
    let mut description = String::new();

    let ty: TypeName = tn!("Nominal");
    let nominal = match contract_iface.global(ty) {
        Ok(values) => values,
        _ => Confined::default(),
    };

    for kv in nominal.iter().cloned() {
        if let StrictVal::Struct(fields) = kv {
            if fields.contains_key(&FieldName::from("ticker")) {
                if let Some(val) = fields.get(&FieldName::from("ticker")) {
                    let val = val.to_string();
                    ticker = val[2..val.len() - 2].to_string();
                };
            }
            if fields.contains_key(&FieldName::from("name")) {
                if let Some(val) = fields.get(&FieldName::from("name")) {
                    let val = val.to_string();
                    name = val[2..val.len() - 2].to_string();
                };
            }
            if fields.contains_key(&FieldName::from("precision")) {
                if let Some(val) = fields.get(&FieldName::from("precision")) {
                    let val = val.to_string();
                    precision = val;
                };
            }
        };
    }

    let ty: TypeName = tn!("ContractText");
    let contract_text = match contract_iface.global(ty) {
        Ok(values) => values,
        _ => Confined::default(),
    };

    for kv in contract_text.iter().cloned() {
        if let StrictVal::Tuple(fields) = kv {
            let val = fields[0].to_string();
            description = val[1..val.len() - 1].to_string();
        };
    }

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

    // Genesis
    let genesis = stock
        .export_contract(ContractId::from_str(&contract_id)?)
        .expect("contract have genesis");

    let genesis_strict = genesis
        .to_strict_serialized::<0xFFFFFF>()
        .expect("invalid genesis data")
        .to_hex();

    let genesis_legacy = encode(
        "rgb",
        genesis
            .to_strict_serialized::<0xFFFFFF>()
            .expect("invalid contract data")
            .to_base32(),
        bech32::Variant::Bech32m,
    )
    .expect("invalid contract data");

    let genesis_formats = GenesisFormats {
        legacy: genesis_legacy,
        strict: genesis_strict,
    };

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
        genesis: genesis_formats,
    };

    Ok(resp)
}
