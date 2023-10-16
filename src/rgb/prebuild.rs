use std::{collections::BTreeMap, ops::Mul, str::FromStr};

use amplify::{
    confinement::{Confined, U32},
    hex::FromHex,
};
use baid58::ToBaid58;
use bech32::{decode, FromBase32};
use bitcoin::Network;
use bitcoin_scripts::address::AddressNetwork;
use garde::Validate;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rgb::{RgbWallet, TerminalPath};
use rgbstd::{
    containers::{Bindle, Consignment},
    contract::ContractId,
    interface::TypedState,
    persistence::{Inventory, Stash, Stock},
};
use rgbwallet::RgbInvoice;
use strict_encoding::{tn, StrictSerialize};

use crate::{
    bitcoin::get_swap_new_address,
    constants::{get_marketplace_fee_percentage, NETWORK},
    structs::{
        AllocationDetail, AllocationValue, AssetType, FullRgbTransferRequest, PsbtFeeRequest,
        PsbtInputRequest, PsbtSigHashRequest, RgbBidRequest, RgbOfferRequest, SecretString,
    },
    validators::RGBContext,
};

use crate::rgb::{
    constants::{BITCOIN_DEFAULT_FETCH_LIMIT, RGB_DEFAULT_FETCH_LIMIT},
    contract::export_contract,
    fs::RgbPersistenceError,
    prefetch::prefetch_resolver_txs,
    prefetch::{
        prefetch_resolver_allocations, prefetch_resolver_user_utxo_status, prefetch_resolver_utxos,
    },
    psbt::estimate_fee_tx,
    resolvers::ExplorerResolver,
    structs::AddressAmount,
    structs::RgbExtractTransfer,
    swap::{extract_transfer as extract_swap_transfer, get_public_offer, RgbBid, RgbOfferSwap},
    transfer::extract_transfer,
    wallet::sync_wallet,
    wallet::{get_address, next_utxos},
    RgbSwapError, SaveTransferError, TransferError,
};

use super::transfer::extract_bindle;

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
                    sigh_hash: None,
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
                    sigh_hash: None,
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
                        sigh_hash: None,
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
            // Increase dust limit to avoid dust change
            let total_spendable = rnd_amount + total_bitcoin_spend + DUST_LIMIT_SATOSHI;
            for utxo in all_unspents {
                if total_spendable < bitcoin_total {
                    break;
                } else {
                    let TerminalPath { app, index } = utxo.derivation.terminal;
                    let btc_input = PsbtInputRequest {
                        descriptor: universal_desc.clone(),
                        utxo: utxo.outpoint.to_string(),
                        utxo_terminal: format!("/{app}/{index}"),
                        tapret: None,
                        sigh_hash: None,
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
        return Err(TransferError::Inflation {
            input: bitcoin_total,
            output: total_spendable,
        });
    } else if change_value > 0 {
        let network = NETWORK.read().await.to_string();
        let network = Network::from_str(&network)
            .map_err(|err| TransferError::WrongNetwork(err.to_string()))?;

        let network = AddressNetwork::from(network);
        // TODO: Use New Address
        let change_address = get_address(1, 0, rgb_wallet.clone(), network)
            .map_err(|err| TransferError::WrongNetwork(err.to_string()))?
            .address;

        let change_bitcoin = format!("{change_address}:{change_value}");
        bitcoin_changes.push(change_bitcoin);
    }

    Ok((assets_inputs, bitcoin_inputs, bitcoin_changes, fee_value))
}

pub async fn prebuild_seller_swap(
    request: RgbOfferRequest,
    stock: &mut Stock,
    rgb_wallet: &mut RgbWallet,
    resolver: &mut ExplorerResolver,
) -> Result<
    (
        Vec<AllocationDetail>,
        Vec<PsbtInputRequest>,
        Vec<PsbtInputRequest>,
        Vec<String>,
        u64,
    ),
    RgbSwapError,
> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .flatten()
            .into_iter()
            .map(|(f, e)| (f, e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let contract_id = ContractId::from_str(&request.contract_id).map_err(|_| {
        let mut errors = BTreeMap::new();
        errors.insert("contract_id".to_string(), "invalid contract id".to_string());
        RgbSwapError::Validation(errors)
    })?;

    let RgbOfferRequest {
        descriptor,
        iface: iface_name,
        contract_amount: target_amount,
        bitcoin_changes,
        ..
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
        .map_err(|_| RgbSwapError::NoContract)?;
    let contract_iface = stock
        .contract_iface(contract_id, iface.iface_id())
        .map_err(|_| RgbSwapError::NoContract)?;

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
        .map_err(RgbSwapError::Export)?;

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
        errors.insert("contract".to_string(), "insufficient state".to_string());
        return Err(RgbSwapError::Validation(errors));
    }

    let asset_unspent_utxos = &mut next_utxos(contract_index, rgb_wallet.clone(), resolver)
        .map_err(|_| RgbSwapError::IO(RgbPersistenceError::RetrieveRgbAccount("".to_string())))?;

    let mut asset_total = 0;
    let mut assets_inputs = vec![];
    let mut assets_allocs = vec![];

    let mut total_asset_bitcoin_unspend: u64 = 0;
    for alloc in allocations.iter() {
        // // TODO: Make more tests!
        // let sig_hash = if assets_inputs.len() <= 0 {
        //     PsbtSigHashRequest::NonePlusAnyoneCanPay
        // } else {
        //     PsbtSigHashRequest::NonePlusAnyoneCanPay
        // };
        let sig_hash = PsbtSigHashRequest::NonePlusAnyoneCanPay;

        match alloc.value {
            AllocationValue::Value(alloc_value) => {
                if asset_total >= target_amount {
                    break;
                }

                let input = PsbtInputRequest {
                    descriptor: universal_desc.clone(),
                    utxo: alloc.utxo.clone(),
                    utxo_terminal: alloc.derivation.to_string(),
                    sigh_hash: Some(sig_hash),
                    tapret: None,
                };
                if !assets_inputs
                    .clone()
                    .into_iter()
                    .any(|x: PsbtInputRequest| x.utxo == alloc.utxo)
                {
                    // let mut empty_input = input.clone();
                    // empty_input.sigh_hash = Some(PsbtSigHashRequest::None);

                    // assets_inputs.push(empty_input);
                    assets_inputs.push(input);
                    assets_allocs.push(alloc.clone());
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
                    utxo_terminal: alloc.derivation.to_string(),
                    sigh_hash: Some(sig_hash),
                    tapret: None,
                };
                if !assets_inputs
                    .clone()
                    .into_iter()
                    .any(|x| x.utxo == alloc.utxo)
                {
                    assets_inputs.push(input);
                    assets_allocs.push(alloc.clone());
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
                RgbSwapError::IO(RgbPersistenceError::RetrieveRgbAccount("".to_string()))
            })?;

        all_unspents.append(&mut unspent_utxos);
    }

    let mut bitcoin_total = total_asset_bitcoin_unspend;
    let total_spendable = total_bitcoin_spend;

    for utxo in all_unspents {
        if bitcoin_total > total_spendable {
            break;
        } else {
            let TerminalPath { app, index } = utxo.derivation.terminal;
            let btc_input = PsbtInputRequest {
                descriptor: universal_desc.clone(),
                utxo: utxo.outpoint.to_string(),
                utxo_terminal: format!("/{app}/{index}"),
                sigh_hash: Some(PsbtSigHashRequest::NonePlusAnyoneCanPay),
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
    if bitcoin_total < total_spendable {
        return Err(RgbSwapError::Inflation {
            input: bitcoin_total,
            output: total_spendable,
        });
    }

    Ok((
        assets_allocs,
        assets_inputs,
        bitcoin_inputs,
        bitcoin_changes,
        change_value,
    ))
}

pub async fn prebuild_buyer_swap(
    sk: &str,
    request: RgbBidRequest,
    rgb_wallet: &mut RgbWallet,
    resolver: &mut ExplorerResolver,
) -> Result<(RgbBid, Vec<PsbtInputRequest>, Vec<String>, u64), RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .flatten()
            .into_iter()
            .map(|(f, e)| (f, e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let RgbBidRequest {
        descriptor,
        offer_id,
        fee,
        asset_amount,
        ..
    } = request;

    let wildcard_terminal = "/*/*";
    let mut universal_desc = descriptor.to_string();
    for contract_type in [AssetType::Bitcoin, AssetType::Change] {
        let contract_index = contract_type as u32;
        let terminal_step = format!("/{contract_index}/*");
        if universal_desc.contains(&terminal_step) {
            universal_desc = universal_desc.replace(&terminal_step, wildcard_terminal);
            break;
        }
    }

    let universal_desc = SecretString(universal_desc);
    let mut all_unspents = vec![];

    // Retrieve Offer
    let offer = get_public_offer(offer_id.clone())
        .await
        .map_err(RgbSwapError::Buyer)?;

    // Retrieve Bitcoin UTXOs
    let mut bitcoin_inputs = vec![];

    let only_bitcoin = [AssetType::Bitcoin, AssetType::Change];
    let derive_indexes = [
        AssetType::Bitcoin,
        AssetType::Change,
        AssetType::RGB20,
        AssetType::RGB21,
    ];
    for derive_type in derive_indexes {
        let derive_index = derive_type.clone() as u32;
        sync_wallet(derive_index, rgb_wallet, resolver);
        prefetch_resolver_utxos(
            derive_index,
            rgb_wallet,
            resolver,
            Some(BITCOIN_DEFAULT_FETCH_LIMIT),
        )
        .await;
        prefetch_resolver_user_utxo_status(derive_index, rgb_wallet, resolver, false).await;

        if only_bitcoin.contains(&derive_type) {
            let mut unspent_utxos = next_utxos(derive_index, rgb_wallet.clone(), resolver)
                .map_err(|_| {
                    RgbSwapError::IO(RgbPersistenceError::RetrieveRgbAccount("".to_string()))
                })?;

            all_unspents.append(&mut unspent_utxos);
        }
    }

    let RgbOfferSwap {
        seller_address,
        bitcoin_price,
        ..
    } = offer;

    let offer_change = format!("{seller_address}:{bitcoin_price}");
    let mut bitcoin_changes = vec![offer_change];

    let mut bitcoin_total = 0;
    let mut total_spendable = bitcoin_price;

    // Swap Fee
    if let Some(swap_fee_address) = get_swap_new_address()
        .await
        .map_err(|op| RgbSwapError::WrongSwapFee(op.to_string()))?
    {
        let swap_fee_perc = get_marketplace_fee_percentage().await;
        let swap_fee_perc = if swap_fee_perc.is_empty() {
            0
        } else {
            swap_fee_perc
                .parse()
                .map_err(|_| RgbSwapError::WrongSwapFee(swap_fee_perc))?
        };
        let total_swap_fee = offer.bitcoin_price.mul(swap_fee_perc) / 100;

        bitcoin_changes.push(format!("{swap_fee_address}:{total_swap_fee}"));
        total_spendable += total_swap_fee;
    }

    // Bitcoin Fees
    let (_, fee_value) = match fee.clone() {
        PsbtFeeRequest::Value(fee_value) => {
            let total_spendable = fee_value + total_spendable;
            for utxo in all_unspents {
                if bitcoin_total > total_spendable {
                    break;
                } else {
                    let TerminalPath { app, index } = utxo.derivation.terminal;
                    let btc_input = PsbtInputRequest {
                        descriptor: universal_desc.clone(),
                        utxo: utxo.outpoint.to_string(),
                        utxo_terminal: format!("/{app}/{index}"),
                        sigh_hash: Some(PsbtSigHashRequest::NonePlusAnyoneCanPay),
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
            // Increase dust limit to avoid dust change
            let total_spendable = total_spendable + DUST_LIMIT_SATOSHI;
            for utxo in all_unspents {
                if total_spendable < bitcoin_total {
                    break;
                } else {
                    let TerminalPath { app, index } = utxo.derivation.terminal;
                    let btc_input = PsbtInputRequest {
                        descriptor: universal_desc.clone(),
                        utxo: utxo.outpoint.to_string(),
                        utxo_terminal: format!("/{app}/{index}"),
                        sigh_hash: Some(PsbtSigHashRequest::NonePlusAnyoneCanPay),
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

            let txids = bitcoin_inputs
                .clone()
                .into_iter()
                .map(|x| bitcoin::Txid::from_str(&x.utxo[..64]).expect("wrong txid"))
                .collect();
            prefetch_resolver_txs(txids, resolver).await;

            let (change_value, fee) = estimate_fee_tx(
                vec![],
                bitcoin_inputs.clone(),
                bitcoin_changes.clone(),
                fee_rate,
                rgb_wallet,
                Some(0),
                None,
                resolver,
            )
            .map_err(RgbSwapError::Estimate)?;

            (change_value, fee)
        }
    };

    let total_spendable = fee_value + offer.bitcoin_price;
    if bitcoin_total < total_spendable {
        return Err(RgbSwapError::Inflation {
            input: bitcoin_total,
            output: total_spendable,
        });
    }

    let bitcoin_utxos = bitcoin_inputs.clone().into_iter().map(|x| x.utxo).collect();
    let new_bid = RgbBid::new(
        sk.to_string(),
        offer_id,
        offer.contract_id.clone(),
        asset_amount,
        offer.bitcoin_price,
        bitcoin_utxos,
    );

    Ok((new_bid, bitcoin_inputs, bitcoin_changes, fee_value))
}

pub fn prebuild_extract_transfer(
    consignment: &str,
) -> Result<RgbExtractTransfer, SaveTransferError> {
    let mut is_armored = false;
    let serialized = if consignment.starts_with("rgb1") {
        let (_, serialized, _) =
            decode(consignment).expect("invalid serialized contract/genesis (bech32m format)");
        Vec::<u8>::from_base32(&serialized)
            .expect("invalid hexadecimal contract/genesis (bech32m format)")
    } else if consignment.starts_with("-----") {
        is_armored = true;
        Vec::new()
    } else {
        Vec::<u8>::from_hex(consignment).expect("invalid hexadecimal contract/genesis")
    };

    if is_armored {
        let transfer =
            Bindle::<Consignment<true>>::from_str(consignment).expect("bindle parse error"); // ?;

        let confined = transfer.to_strict_serialized::<U32>()?;

        let (tx_id, transfer, offer_id, bid_id) = match extract_bindle(transfer) {
            Ok((txid, transfer)) => (txid, transfer, None, None),
            _ => match extract_swap_transfer(consignment.to_owned()) {
                Ok((txid, transfer, offer_id, bid_id)) => (
                    txid,
                    transfer,
                    Some(offer_id.to_baid58_string()),
                    Some(bid_id.to_baid58_string()),
                ),
                Err(err) => return Err(SaveTransferError::WrongConsigSwap(err)),
            },
        };

        let contract_id = transfer.contract_id().to_string();
        Ok(RgbExtractTransfer {
            consig_id: transfer.id().to_string(),
            contract_id,
            tx_id,
            transfer,
            offer_id,
            bid_id,
            strict: confined,
        })
    } else {
        let confined = Confined::try_from_iter(serialized.iter().copied())
            .expect("invalid confined serialization");
        let (tx_id, transfer, offer_id, bid_id) = match extract_transfer(consignment.to_owned()) {
            Ok((txid, transfer)) => (txid, transfer, None, None),
            _ => match extract_swap_transfer(consignment.to_owned()) {
                Ok((txid, transfer, offer_id, bid_id)) => (
                    txid,
                    transfer,
                    Some(offer_id.to_baid58_string()),
                    Some(bid_id.to_baid58_string()),
                ),
                Err(err) => return Err(SaveTransferError::WrongConsigSwap(err)),
            },
        };

        let contract_id = transfer.contract_id().to_string();
        Ok(RgbExtractTransfer {
            consig_id: transfer.id().to_string(),
            contract_id,
            tx_id,
            transfer,
            offer_id,
            bid_id,
            strict: confined,
        })
    }
}
