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

use crate::rgb::{resolvers::ResolveSpent, wallet::allocations_by_contract};
use crate::structs::{
    AllocationValue, ContractFormats, ContractMeta, ContractMetadata, ContractResponse,
    GenesisFormats, MediaInfo, UDADetail,
};

// TODO: Create one extractor by contract interface
pub fn extract_contract_by_id<T>(
    contract_id: ContractId,
    stock: &mut Stock,
    resolver: &mut T,
    wallet: &mut Option<RgbWallet>,
) -> Result<ContractResponse, anyhow::Error>
where
    T: ResolveSpent + Resolver,
{
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
    )?;

    let contract_strict = contract_bindle.to_strict_serialized::<0xFFFFFF>()?.to_hex();

    let contract_iface = stock
        .contract_iface(contract_bindle.contract_id(), iface_id.to_owned())
        .expect("invalid contracts state");

    let mut ticker = String::new();
    let mut name = String::new();
    let mut precision: u8 = 0;
    let mut description = String::new();

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
            }
            if let Some(StrictVal::Enum(en)) = fields.get(&FieldName::from("precision")) {
                let val = en.unwrap_ord();
                precision = val;
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

    let iface_index = match iface.name.as_str() {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 9,
    };

    let mut balance = 0;
    let mut allocations = vec![];
    let contract_id = ContractId::from_str(&contract_id)?;
    if let Some(wallet) = wallet {
        let watcher = allocations_by_contract(contract_id, iface_index, wallet, stock, resolver)
            .expect("invalid allocation states");

        allocations = watcher.allocations;
        balance = allocations
            .clone()
            .into_iter()
            .filter(|a| a.is_mine)
            .filter(|a| !a.is_spent)
            .map(|a| match a.value {
                AllocationValue::Value(value) => value.to_owned(),
                AllocationValue::UDA(_) => 1,
            })
            .sum();
    }

    // Genesis
    let contract_genesis = stock
        .export_contract(contract_id)
        .expect("contract have genesis");

    let mut supply = 0;
    for (index, (_, global_assign)) in contract_genesis.genesis.assignments.iter().enumerate() {
        let idx = index as u16;
        if global_assign.is_fungible() {
            if let Some(reveal) = global_assign.as_fungible_state_at(idx)? {
                supply += reveal.value.as_u64();
            }
        } else if global_assign.is_structured()
            && global_assign.as_structured_state_at(idx)?.is_some()
        {
            supply += 1;
        }
    }

    let genesis = contract_genesis.genesis.clone();
    let genesis_strict = genesis.to_strict_serialized::<0xFFFFFF>()?.to_hex();

    let genesis_legacy = encode(
        "rgb",
        genesis.to_strict_serialized::<0xFFFFFF>()?.to_base32(),
        bech32::Variant::Bech32m,
    )?;

    let genesis_formats = GenesisFormats {
        legacy: genesis_legacy,
        strict: genesis_strict,
        armored: "".to_string(),
    };

    // Only RGB21/UDA
    let mut meta = none!();
    let ty: FieldName = FieldName::from("tokens");
    if contract_iface.global(ty.clone()).is_ok() {
        let type_id = contract_iface
            .iface
            .global_type(&ty)
            .expect("no global type id");

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
                    source: String::from_utf8(preview.data.to_inner())?,
                };
            }

            let single = ContractMetadata::UDA(UDADetail {
                token_index: token_data.index.to_string().parse()?,
                ticker: ticker.clone(),
                name: name.clone(),
                description: description.clone(),
                balance,
                media: vec![media],
                allocations: allocations.clone(),
            });

            meta = Some(ContractMeta::with(single));
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

            meta = Some(ContractMeta::with(ContractMetadata::Collectible(
                collectibles,
            )));
        }
    }

    let resp = ContractResponse {
        contract_id: contract_id.to_string(),
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
