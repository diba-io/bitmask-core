use std::{collections::BTreeMap, str::FromStr};

use bitcoin::Network;
use bitcoin_scripts::address::AddressNetwork;
use garde::Validate;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rgb::{RgbWallet, TerminalPath};
use rgbstd::{
    contract::ContractId,
    interface::TypedState,
    persistence::{Inventory, Stash, Stock},
};
use rgbwallet::RgbInvoice;
use strict_encoding::tn;

use crate::{
    constants::NETWORK,
    structs::{
        AllocationDetail, AllocationValue, AssetType, FullRgbTransferRequest, PsbtFeeRequest,
        PsbtInputRequest, SecretString,
    },
    validators::RGBContext,
};

use crate::rgb::{
    constants::{BITCOIN_DEFAULT_FETCH_LIMIT, RGB_DEFAULT_FETCH_LIMIT},
    contract::export_contract,
    fs::RgbPersistenceError,
    prefetch::{
        prefetch_resolver_allocations, prefetch_resolver_user_utxo_status, prefetch_resolver_utxos,
    },
    resolvers::ExplorerResolver,
    structs::AddressAmount,
    wallet::sync_wallet,
    wallet::{get_address, next_utxos},
    TransferError,
};

pub async fn prebuild_transfer_asset(
    request: FullRgbTransferRequest,
    stock: &mut Stock,
    rgb_wallet: &mut RgbWallet,
    resolver: &mut ExplorerResolver,
) -> Result<(Vec<PsbtInputRequest>, Vec<PsbtInputRequest>, Vec<String>), TransferError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .flatten()
            .into_iter()
            .map(|(f, e)| (f, e.to_string()))
            .collect();
        return Err(TransferError::Validation(errors));
    }

    let contract_id = ContractId::from_str(&request.contract_id).map_err(|_| {
        let mut errors = BTreeMap::new();
        errors.insert("contract_id".to_string(), "invalid contract id".to_string());
        TransferError::Validation(errors)
    })?;

    let invoice = RgbInvoice::from_str(&request.rgb_invoice).map_err(|_| {
        let mut errors = BTreeMap::new();
        errors.insert(
            "rgb_invoice".to_string(),
            "invalid rgb invoice data".to_string(),
        );
        TransferError::Validation(errors)
    })?;

    let target_amount = match invoice.owned_state {
        TypedState::Amount(target_amount) => target_amount,
        _ => {
            let mut errors = BTreeMap::new();
            errors.insert(
                "rgb_invoice".to_string(),
                "invalid rgb invoice data".to_string(),
            );
            return Err(TransferError::Validation(errors));
        }
    };

    let FullRgbTransferRequest {
        contract_id: _,
        iface: iface_name,
        rgb_invoice: _,
        descriptor,
        change_terminal: _,
        fee,
        mut bitcoin_changes,
    } = request;

    let wildcard_terminal = "/*/*";
    let mut universal_desc = descriptor.to_string();
    for contract_type in [
        AssetType::RGB20,
        AssetType::RGB21,
        AssetType::Contract,
        AssetType::Bitcoin,
    ] {
        let contract_index = contract_type as u32;
        let terminal_step = format!("/{contract_index}/*");
        if universal_desc.contains(&terminal_step) {
            universal_desc = universal_desc.replace(&terminal_step, wildcard_terminal);
            break;
        }
    }
    let mut all_unspents = vec![];

    // Get All Assets UTXOs
    let contract_index = if let "RGB20" = iface_name.as_str() {
        AssetType::RGB20
    } else {
        AssetType::RGB21
    };

    let iface = stock
        .iface_by_name(&tn!(iface_name))
        .map_err(|_| TransferError::NoIface)?;
    let contract_iface = stock
        .contract_iface(contract_id, iface.iface_id())
        .map_err(|_| TransferError::NoContract)?;

    let contract_index = contract_index as u32;
    prefetch_resolver_allocations(contract_iface, resolver).await;
    sync_wallet(contract_index, rgb_wallet, resolver);
    prefetch_resolver_utxos(
        contract_index,
        rgb_wallet,
        resolver,
        Some(RGB_DEFAULT_FETCH_LIMIT),
    )
    .await;

    let contract = export_contract(contract_id, stock, resolver, &mut Some(rgb_wallet.clone()))
        .map_err(TransferError::Export)?;

    let allocations: Vec<AllocationDetail> = contract
        .allocations
        .into_iter()
        .filter(|x| x.is_mine && !x.is_spent)
        .collect();

    let asset_total: u64 = allocations
        .clone()
        .into_iter()
        .filter(|a| a.is_mine && !a.is_spent)
        .map(|a| match a.value {
            AllocationValue::Value(value) => value.to_owned(),
            AllocationValue::UDA(_) => 1,
        })
        .sum();

    if asset_total < target_amount {
        let mut errors = BTreeMap::new();
        errors.insert("rgb_invoice".to_string(), "insufficient state".to_string());
        return Err(TransferError::Validation(errors));
    }

    let asset_unspent_utxos = &mut next_utxos(contract_index, rgb_wallet.clone(), resolver)
        .map_err(|_| TransferError::IO(RgbPersistenceError::RetrieveRgbAccount))?;

    let mut asset_total = 0;
    let mut asset_inputs = vec![];
    let mut rng = StdRng::seed_from_u64(1);
    let rnd_amount = rng.gen_range(600..1500);

    let mut total_asset_bitcoin_unspend: u64 = 0;
    for alloc in allocations.into_iter() {
        match alloc.value {
            AllocationValue::Value(alloc_value) => {
                if asset_total >= target_amount {
                    break;
                }

                let input = PsbtInputRequest {
                    descriptor: SecretString(universal_desc.clone()),
                    utxo: alloc.utxo.clone(),
                    utxo_terminal: alloc.derivation,
                    tapret: None,
                };
                if !asset_inputs
                    .clone()
                    .into_iter()
                    .any(|x: PsbtInputRequest| x.utxo == alloc.utxo)
                {
                    asset_inputs.push(input);
                    total_asset_bitcoin_unspend += asset_unspent_utxos
                        .clone()
                        .into_iter()
                        .filter(|x| {
                            x.outpoint.to_string() == alloc.utxo.clone()
                                && alloc.is_mine
                                && !alloc.is_spent
                        })
                        .map(|x| x.amount)
                        .sum::<u64>();
                    asset_total += alloc_value;
                }
            }
            AllocationValue::UDA(_) => {
                let input = PsbtInputRequest {
                    descriptor: SecretString(universal_desc.clone()),
                    utxo: alloc.utxo.clone(),
                    utxo_terminal: alloc.derivation,
                    tapret: None,
                };
                if !asset_inputs
                    .clone()
                    .into_iter()
                    .any(|x| x.utxo == alloc.utxo)
                {
                    asset_inputs.push(input);
                    total_asset_bitcoin_unspend += asset_unspent_utxos
                        .clone()
                        .into_iter()
                        .filter(|x| {
                            x.outpoint.to_string() == alloc.utxo.clone()
                                && alloc.is_mine
                                && !alloc.is_spent
                        })
                        .map(|x| x.amount)
                        .sum::<u64>();
                }
                break;
            }
        }
    }

    // Get All Bitcoin UTXOs
    let total_bitcoin_spend: u64 = bitcoin_changes
        .clone()
        .into_iter()
        .map(|x| {
            let recipient = AddressAmount::from_str(&x).expect("invalid address amount format");
            recipient.amount
        })
        .sum();
    let mut bitcoin_inputs = vec![];
    if let PsbtFeeRequest::Value(fee_amount) = fee.clone() {
        let bitcoin_indexes = [0, 1];
        for bitcoin_index in bitcoin_indexes {
            sync_wallet(bitcoin_index, rgb_wallet, resolver);
            prefetch_resolver_utxos(
                bitcoin_index,
                rgb_wallet,
                resolver,
                Some(BITCOIN_DEFAULT_FETCH_LIMIT),
            )
            .await;
            prefetch_resolver_user_utxo_status(bitcoin_index, rgb_wallet, resolver, false).await;

            let mut unspent_utxos = next_utxos(bitcoin_index, rgb_wallet.clone(), resolver)
                .map_err(|_| TransferError::IO(RgbPersistenceError::RetrieveRgbAccount))?;

            all_unspents.append(&mut unspent_utxos);
        }

        let mut bitcoin_total = total_asset_bitcoin_unspend;
        for utxo in all_unspents {
            if bitcoin_total > (fee_amount + rnd_amount) {
                break;
            } else {
                bitcoin_total += utxo.amount;

                let TerminalPath { app, index } = utxo.derivation.terminal;
                let btc_input = PsbtInputRequest {
                    descriptor: SecretString(universal_desc.clone()),
                    utxo: utxo.outpoint.to_string(),
                    utxo_terminal: format!("/{app}/{index}"),
                    tapret: None,
                };
                if !bitcoin_inputs
                    .clone()
                    .into_iter()
                    .any(|x: PsbtInputRequest| x.utxo == utxo.outpoint.to_string())
                {
                    bitcoin_inputs.push(btc_input);
                }
            }
        }
        if bitcoin_total < (fee_amount + rnd_amount + total_bitcoin_spend) {
            let mut errors = BTreeMap::new();
            errors.insert("bitcoin".to_string(), "insufficient satoshis".to_string());
            return Err(TransferError::Validation(errors));
        } else {
            let network = NETWORK.read().await.to_string();
            let network = Network::from_str(&network)
                .map_err(|err| TransferError::WrongNetwork(err.to_string()))?;

            let network = AddressNetwork::from(network);

            let change_address = get_address(1, 1, rgb_wallet.clone(), network)
                .map_err(|err| TransferError::WrongNetwork(err.to_string()))?
                .address;

            let change_amount = bitcoin_total - (rnd_amount + fee_amount + total_bitcoin_spend);
            let change_bitcoin = format!("{change_address}:{change_amount}");
            bitcoin_changes.push(change_bitcoin);
        }
    }

    Ok((asset_inputs, bitcoin_inputs, bitcoin_changes))
}
