use std::str::FromStr;

use amplify::{confinement::Confined, hex::ToHex};
use bech32::{encode, ToBase32};
use rgb::{Resolver, RgbWallet};
use rgbstd::{
    contract::ContractId,
    interface::{rgb21::TokenData, IfaceId, IfacePair},
    persistence::{Inventory, Stock},
};
use strict_encoding::{FieldName, StrictDeserialize, StrictSerialize};
use strict_types::StrictVal;

use crate::structs::{
    AllocationValue, ContractFormats, ContractMeta, ContractMetadata, ContractResponse, MediaInfo,
    UDADetail,
};
use crate::{rgb::wallet::list_allocations, structs::GenesisFormats};

// TODO: Create one extractor by contract interface
pub fn extract_contract_by_id(
    contract_id: ContractId,
    stock: &mut Stock,
    resolver: &mut impl Resolver,
    wallet: &mut Option<RgbWallet>,
) -> Result<ContractResponse, anyhow::Error> {
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
    let mut supply = 0;

    let ty: FieldName = FieldName::from("spec");
    let nominal = match contract_iface.global(ty) {
        Ok(values) => values,
        _ => Confined::default(),
    };

    for kv in nominal.iter().cloned() {
        if let StrictVal::Struct(fields) = kv {
            if let Some(StrictVal::Struct(naming)) = fields.get(&FieldName::from("naming")) {
                if naming.contains_key(&FieldName::from("ticker")) {
                    if let Some(val) = naming.get(&FieldName::from("ticker")) {
                        let val = val.to_string();
                        ticker = val[2..val.len() - 2].to_string();
                    };
                }
                if naming.contains_key(&FieldName::from("name")) {
                    if let Some(val) = naming.get(&FieldName::from("name")) {
                        let val = val.to_string();
                        name = val[2..val.len() - 2].to_string();
                    };
                }
                if naming.contains_key(&FieldName::from("precision")) {
                    if let Some(val) = naming.get(&FieldName::from("precision")) {
                        let val = val.to_string();
                        precision = val;
                    };
                }
            }
        };
    }

    let ty: FieldName = FieldName::from("terms");
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

    for owned in &contract_iface.iface.assignments {
        if let Ok(allocations) = contract_iface.fungible(owned.name.clone()) {
            for allocation in allocations {
                supply = allocation.value;
            }
        }

        if let Ok(allocations) = contract_iface.data(owned.name.clone()) {
            for _ in allocations {
                supply += 1;
            }
        }
    }
    let mut balance = 0;
    let mut allocations = vec![];

    // TODO: workaround
    let iface_index = match iface.name.as_str() {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 9,
    };

    if let Some(wallet) = wallet {
        let watcher = list_allocations(wallet, stock, iface_index, resolver)
            .expect("invalid allocation states");
        if let Some(mut watcher_detail) = watcher.into_iter().find(|w| w.contract_id == contract_id)
        {
            allocations.append(&mut watcher_detail.allocations);
        }

        balance = allocations
            .clone()
            .into_iter()
            .filter(|a| a.is_mine)
            .map(|a| match a.value {
                AllocationValue::Value(value) => value.to_owned(),
                AllocationValue::UDA(_) => 1,
            })
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
        armored: genesis.to_string(),
    };

    // Only RGB21/UDA
    let mut meta = none!();
    let ty: FieldName = FieldName::from("tokens");
    if contract_iface.global(ty.clone()).is_ok() {
        let type_id = contract_iface.iface.global_type(&ty).expect("");

        let type_schema = contract_iface
            .state
            .schema
            .global_types
            .get(&type_id)
            .expect("invalid type schema");

        let state = unsafe { contract_iface.state.global_unchecked(type_id) };

        let state = state
            .into_iter()
            .map(|revealed| {
                let ast_data = revealed.as_ref().to_owned();
                TokenData::from_strict_serialized(ast_data)
            })
            .take(type_schema.max_items as usize)
            .collect::<Result<Vec<_>, _>>()?;
        let tokens_data = state;

        if tokens_data.len() <= 1 {
            let token_data = tokens_data[0].clone();
            let mut media = MediaInfo::default();
            if let Some(preview) = token_data.preview {
                media = MediaInfo {
                    ty: preview.ty.to_string(),
                    source: String::from_utf8(preview.data.to_inner()).expect("invalid data"),
                };
            }

            let single = ContractMetadata::UDA(UDADetail {
                token_index: token_data
                    .index
                    .to_string()
                    .parse()
                    .expect("invalid token_index"),
                ticker: ticker.clone(),
                name: name.clone(),
                description: description.clone(),
                balance,
                media: vec![media],
                allocations: allocations.clone(),
            });

            meta = ContractMeta::with(single);
        } else {
            let collectibles = tokens_data
                .into_iter()
                .map(|token_data| {
                    let mut media = MediaInfo::default();
                    if let Some(preview) = token_data.preview.to_owned() {
                        media = MediaInfo {
                            ty: preview.ty.to_string(),
                            source: String::from_utf8(preview.data.to_inner())
                                .expect("invalid data"),
                        };
                    }

                    let mut token_ticker = String::new();
                    let mut token_name = String::new();
                    let mut token_description = String::new();

                    if let Some(ticker) = token_data.ticker {
                        token_ticker = ticker.to_string();
                    }

                    if let Some(name) = token_data.name {
                        token_name = name.to_string();
                    }

                    if let Some(details) = token_data.details {
                        token_description = details.to_string();
                    }

                    let mut token_alloc = vec![];
                    for alloc in allocations.clone().into_iter() {
                        if let AllocationValue::UDA(position) = &alloc.value {
                            let token_index: u32 = token_data
                                .index
                                .to_string()
                                .parse()
                                .expect("invalid token_index");
                            if position.token_index == token_index {
                                token_alloc.push(alloc);
                            }
                        }
                    }

                    UDADetail {
                        token_index: token_data
                            .index
                            .to_string()
                            .parse()
                            .expect("invalid token_index"),
                        ticker: token_ticker,
                        name: token_name,
                        description: token_description,
                        balance,
                        media: vec![media],
                        allocations: token_alloc,
                    }
                })
                .collect();

            meta = ContractMeta::with(ContractMetadata::Collectible(collectibles));
        }
    }

    let resp = ContractResponse {
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
        meta,
    };

    Ok(resp)
}
