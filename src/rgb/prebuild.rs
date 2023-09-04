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
    psbt::estimate_fee_tx,
    resolvers::ExplorerResolver,
    structs::AddressAmount,
    wallet::sync_wallet,
    wallet::{get_address, next_utxos},
    TransferError,
};

use super::prefetch::prefetch_resolver_txs;

pub const DUST_LIMIT_SATOSHI: u64 = 546;

pub async fn prebuild_transfer_asset(
    request: FullRgbTransferRequest,
    stock: &mut Stock,
    rgb_wallet: &mut RgbWallet,
    resolver: &mut ExplorerResolver,
) -> Result<
    (
        Vec<PsbtInputRequest>,
        Vec<PsbtInputRequest>,
        Vec<String>,
        u64,
    ),
    TransferError,
> {
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
        change_terminal,
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
        AssetType::Change,
    ] {
        let contract_index = contract_type as u32;
        let terminal_step = format!("/{contract_index}/*");
        if universal_desc.contains(&terminal_step) {
            universal_desc = universal_desc.replace(&terminal_step, wildcard_terminal);
            break;
        }
    }

    let universal_desc = SecretString(universal_desc);
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
    sync_wallet(contract_index, rgb_wallet, resolver);
    prefetch_resolver_utxos(
        contract_index,
        rgb_wallet,
        resolver,
        Some(RGB_DEFAULT_FETCH_LIMIT),
    )
    .await;
    prefetch_resolver_allocations(contract_iface, resolver).await;

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
        .map_err(|_| TransferError::IO(RgbPersistenceError::RetrieveRgbAccount("".to_string())))?;

    let mut asset_total = 0;
    let mut assets_inputs = vec![];
    let mut rng = StdRng::from_entropy();
    let rnd_amount = rng.gen_range(600..1500);

    let mut total_asset_bitcoin_unspend: u64 = 0;
    for alloc in allocations.into_iter() {
        match alloc.value {
            AllocationValue::Value(alloc_value) => {
                if asset_total >= target_amount {
                    break;
                }

                let input = PsbtInputRequest {
                    descriptor: universal_desc.clone(),
                    utxo: alloc.utxo.clone(),
                    utxo_terminal: alloc.derivation,
                    tapret: None,
                };
                if !assets_inputs
                    .clone()
                    .into_iter()
                    .any(|x: PsbtInputRequest| x.utxo == alloc.utxo)
                {
                    assets_inputs.push(input);
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
                    descriptor: universal_desc.clone(),
                    utxo: alloc.utxo.clone(),
                    utxo_terminal: alloc.derivation,
                    tapret: None,
                };
                if !assets_inputs
                    .clone()
                    .into_iter()
                    .any(|x| x.utxo == alloc.utxo)
                {
                    assets_inputs.push(input);
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

        let mut unspent_utxos =
            next_utxos(bitcoin_index, rgb_wallet.clone(), resolver).map_err(|_| {
                TransferError::IO(RgbPersistenceError::RetrieveRgbAccount("".to_string()))
            })?;

        all_unspents.append(&mut unspent_utxos);
    }

    let mut bitcoin_total = total_asset_bitcoin_unspend;
    let (change_value, fee_value) = match fee.clone() {
        PsbtFeeRequest::Value(fee_value) => {
            let total_spendable = fee_value + rnd_amount + total_bitcoin_spend;
            for utxo in all_unspents {
                if bitcoin_total > total_spendable {
                    break;
                } else {
                    let TerminalPath { app, index } = utxo.derivation.terminal;
                    let btc_input = PsbtInputRequest {
                        descriptor: universal_desc.clone(),
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
                        bitcoin_total += utxo.amount;
                    }
                }
            }

            let change_value = bitcoin_total - total_spendable;
            (change_value, fee_value)
        }
        PsbtFeeRequest::FeeRate(fee_rate) => {
            let total_spendable = rnd_amount + total_bitcoin_spend;
            for utxo in all_unspents {
                if bitcoin_total > total_spendable {
                    break;
                } else {
                    let TerminalPath { app, index } = utxo.derivation.terminal;
                    let btc_input = PsbtInputRequest {
                        descriptor: universal_desc.clone(),
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
                        bitcoin_total += utxo.amount;
                    }
                }
            }

            let mut all_inputs = assets_inputs.clone();
            all_inputs.extend(bitcoin_inputs.clone());

            let txids = all_inputs
                .clone()
                .into_iter()
                .map(|x| bitcoin::Txid::from_str(&x.utxo[..64]).expect("wrong txid"))
                .collect();
            prefetch_resolver_txs(txids, resolver).await;

            let (change_value, fee) = estimate_fee_tx(
                assets_inputs.clone(),
                bitcoin_inputs.clone(),
                bitcoin_changes.clone(),
                fee_rate,
                rgb_wallet,
                Some(rnd_amount),
                Some(change_terminal),
                resolver,
            )
            .map_err(TransferError::Estimate)?;

            (change_value, fee)
        }
    };

    let total_spendable = fee_value + rnd_amount + total_bitcoin_spend;
    if bitcoin_total < total_spendable {
        let mut errors = BTreeMap::new();
        errors.insert("bitcoin".to_string(), "insufficient satoshis".to_string());
        return Err(TransferError::Validation(errors));
    } else if change_value > 0 {
        let network = NETWORK.read().await.to_string();
        let network = Network::from_str(&network)
            .map_err(|err| TransferError::WrongNetwork(err.to_string()))?;

        let network = AddressNetwork::from(network);
        let change_address = get_address(1, 0, rgb_wallet.clone(), network)
            .map_err(|err| TransferError::WrongNetwork(err.to_string()))?
            .address;

        let change_bitcoin = format!("{change_address}:{change_value}");
        bitcoin_changes.push(change_bitcoin);
    }

    Ok((assets_inputs, bitcoin_inputs, bitcoin_changes, fee_value))
}
