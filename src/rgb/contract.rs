use amplify::{confinement::U32, hex::ToHex};
use bech32::{encode, ToBase32};
use rgb::{Resolver, RgbWallet};
use rgbstd::{
    contract::ContractId,
    interface::{rgb21::TokenData, IfaceId, IfacePair},
    persistence::{Inventory, InventoryInconsistency, StashInconsistency, Stock},
    stl::{ContractData, DivisibleAssetSpec, RicardianContract},
};
use strict_encoding::{FieldName, StrictDeserialize, StrictSerialize};

use crate::structs::{
    AllocationValue, ContractFormats, ContractMeta, ContractMetadata, ContractResponse,
    GenesisFormats, MediaInfo, UDADetail,
};
use crate::{
    rgb::{resolvers::ResolveSpent, wallet::contract_allocations},
    structs::AttachInfo,
};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ExportContractError {
    /// The contract {0} is not found
    NoContrat(String),
    /// The contract {0} presents stash inconstency ({1})
    StashInconsistency(String, String),
    /// The contract {0} presents strict inconstency ({1})
    StrictInconsistency(String, String),
    /// The genesis of the contract {0} presents inconstency ({1})
    GenesisInconsistency(String, String),
    /// The contract {0} cannot be converted to {1}
    ContractFormat(String, String),
    /// The the contract {0} cannot have {1} global data
    GlobalNotFound(String, String),
}

// TODO: Create one extractor by contract interface
pub fn export_contract<T>(
    contract_id: ContractId,
    stock: &mut Stock,
    resolver: &mut T,
    wallet: &mut Option<RgbWallet>,
) -> Result<ContractResponse, ExportContractError>
where
    T: ResolveSpent + Resolver,
{
    let contract_bindle = stock
        .export_contract(contract_id)
        .or(Err(ExportContractError::NoContrat(contract_id.to_string())))?;

    let contr_id = contract_id.to_string();
    let ifaces: Vec<IfaceId> = contract_bindle
        .ifaces
        .keys()
        .map(|f| f.to_owned())
        .collect();

    let iface_id = ifaces[0];
    let (iface, iimpl) = match &contract_bindle.ifaces.get(&iface_id) {
        Some(IfacePair { iface, iimpl }) => (iface, iimpl),
        _ => {
            return Err(ExportContractError::StashInconsistency(
                contr_id,
                InventoryInconsistency::Stash(StashInconsistency::IfaceAbsent(iface_id))
                    .to_string(),
            ))
        }
    };

    let iimpl_id = iimpl.impl_id().to_string();
    // Formats
    let contract_serialized = match contract_bindle.to_strict_serialized::<U32>() {
        Ok(serialized) => serialized,
        Err(err) => {
            return Err(ExportContractError::StrictInconsistency(
                contr_id,
                err.to_string(),
            ))
        }
    };
    let contract_strict = contract_serialized.to_hex();
    let contract_legacy = match encode(
        "rgb",
        contract_serialized.to_base32(),
        bech32::Variant::Bech32m,
    ) {
        Ok(legacy) => legacy,
        _ => {
            return Err(ExportContractError::StrictInconsistency(
                contr_id,
                "There was a problem converting baid58 to bench32".to_string(),
            ))
        }
    };

    let genesis_serialized = match contract_bindle.genesis.to_strict_serialized::<U32>() {
        Ok(serialized) => serialized,
        _ => {
            return Err(ExportContractError::ContractFormat(
                contr_id,
                "bench32".to_string(),
            ))
        }
    };
    let genesis_strict = genesis_serialized.to_hex();
    let genesis_legacy = match encode(
        "rgb",
        genesis_serialized.to_base32(),
        bech32::Variant::Bech32m,
    ) {
        Ok(legacy) => legacy,
        _ => {
            return Err(ExportContractError::ContractFormat(
                contr_id,
                "bench32".to_string(),
            ))
        }
    };

    let contract_iface = stock
        .contract_iface(contract_id, iface_id.to_owned())
        .expect("invalid contracts state");

    let ty: FieldName = FieldName::from("spec");
    let specs = match contract_iface.global(ty) {
        Ok(values) => DivisibleAssetSpec::from_strict_val_unchecked(&values[0]),
        Err(err) => {
            return Err(ExportContractError::StrictInconsistency(
                contr_id,
                err.to_string(),
            ))
        }
    };

    let mut description = String::new();
    let ty_data: FieldName = FieldName::from("data");
    if let Ok(values) = contract_iface.global(ty_data) {
        let contract = ContractData::from_strict_val_unchecked(&values[0]);
        description = contract.terms.to_string();
    };

    let ty_terms: FieldName = FieldName::from("terms");
    if let Ok(values) = contract_iface.global(ty_terms) {
        let contract = RicardianContract::from_strict_val_unchecked(&values[0]);
        description = contract.to_string();
    };

    let iface_index = match iface.name.as_str() {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 9,
    };

    let mut balance = 0;
    let mut allocations = vec![];
    if let Some(wallet) = wallet {
        let watcher = contract_allocations(contract_id, iface_index, wallet, stock, resolver)
            .expect("invalid allocation states");

        allocations = watcher.allocations;
        balance = allocations
            .clone()
            .into_iter()
            .filter(|a| a.is_mine && !a.is_spent)
            .map(|a| match a.value {
                AllocationValue::Value(value) => value.to_owned(),
                AllocationValue::UDA(_) => 1,
            })
            .sum();
    }

    let mut supply = 0;
    for (index, (_, global_assign)) in contract_bindle.genesis.assignments.iter().enumerate() {
        let idx = index as u16;
        if global_assign.is_fungible() {
            if let Ok(Some(reveal)) = global_assign.as_fungible_state_at(idx) {
                supply += reveal.value.as_u64();
            }
        } else if global_assign.is_structured() && global_assign.as_structured_state_at(idx).is_ok()
        {
            supply += 1;
        }
    }

    // Only RGB21/UDA
    let mut meta = none!();
    let ty: FieldName = FieldName::from("tokens");
    if contract_iface.global(ty.clone()).is_ok() {
        if let Some(type_id) = contract_iface.iface.global_type(&ty) {
            let state = unsafe { contract_iface.state.global_unchecked(type_id) };

            let type_schema = contract_iface
                .state
                .schema
                .global_types
                .get(&type_id)
                .expect("invalid type schema");

            let state = state
                .into_iter()
                .map(|revealed| {
                    let ast_data = revealed.as_ref().to_owned();
                    TokenData::from_strict_serialized(ast_data)
                })
                .take(type_schema.max_items as usize)
                .collect::<Result<Vec<_>, _>>();

            let tokens_data = match state {
                Ok(tokens_data) => tokens_data,
                Err(err) => {
                    return Err(ExportContractError::StrictInconsistency(
                        contr_id,
                        err.to_string(),
                    ))
                }
            };

            if tokens_data.len() <= 1 {
                let token_data = tokens_data[0].clone();
                let mut media = MediaInfo::default();
                if let Some(preview) = token_data.preview {
                    media = MediaInfo {
                        ty: preview.ty.to_string(),
                        source: String::from_utf8(preview.data.to_inner())
                            .expect("invalid media_info data"),
                    };
                }

                let mut attach = None;
                if let Some(att) = token_data.media {
                    attach = Some(AttachInfo {
                        ty: att.ty.to_string(),
                        source: att.digest.to_hex(),
                    });
                }

                let single = ContractMetadata::UDA(UDADetail {
                    token_index: token_data
                        .index
                        .to_string()
                        .parse()
                        .expect("invalid token_index"),
                    ticker: specs.ticker().into(),
                    name: specs.name().into(),
                    description: specs.details().unwrap_or_default().into(),
                    balance,
                    media: vec![media],
                    attach,
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
                            attach: None,
                            allocations: token_alloc,
                        }
                    })
                    .collect();

                meta = Some(ContractMeta::with(ContractMetadata::Collectible(
                    collectibles,
                )));
            }
        }
    }

    let resp = ContractResponse {
        contract_id: contr_id,
        iimpl_id,
        iface: iface.name.to_string(),
        ticker: specs.ticker().into(),
        name: specs.name().into(),
        description,
        precision: 0,
        supply,
        balance,
        allocations,
        contract: ContractFormats {
            legacy: contract_legacy,
            strict: contract_strict,
            armored: contract_bindle.to_string(),
        },
        genesis: GenesisFormats {
            legacy: genesis_legacy,
            strict: genesis_strict,
            armored: "".to_string(),
        },
        meta,
    };

    Ok(resp)
}
