use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use amplify::hex::ToHex;
use anyhow::{anyhow, Result};
use bdk::{descriptor::Descriptor, LocalUtxo};
use bitcoin::{psbt::serialize::Serialize, OutPoint};
use bp::seals::txout::{blind::RevealedSeal, CloseMethod, ExplicitSeal};
use electrum_client::{Client, ElectrumApi};
use regex::Regex;
use rgb20::Asset;
use rgb_core::{Anchor, IntoRevealedSeal};
use rgb_std::{
    blank::BlankBundle,
    fungible::allocation::{AllocatedValue, UtxobValue},
    psbt::{RgbExt, RgbInExt},
    AssignedState, Contract, Disclosure, InmemConsignment, Node, SealEndpoint, TransferConsignment,
    TransitionBundle,
};
use strict_encoding::StrictEncode;
use wallet::{
    descriptors::InputDescriptor, locks::LockTime, psbt::Psbt, scripts::taproot::DfsPath,
};

use crate::{
    data::{
        constants::BITCOIN_ELECTRUM_API,
        structs::{FullCoin, FullUtxo, SealCoins, TransferAssetsNativeResponse},
    },
    debug, error, info,
    rgb::shared::{compose_consignment, outpoint_state, ConsignmentDetails},
    trace,
};

pub async fn transfer_asset(
    bdk_rgb_assets_descriptor_xpub: &str,
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str, // rgbc1...
    asset_utxos: Vec<FullUtxo>,
) -> Result<TransferAssetsNativeResponse> {
    debug!(format!("asset_contract: {asset_contract}"));

    // rgb-cli -n testnet transfer compose ${CONTRACT_ID} ${UTXO_SRC} ${CONSIGNMENT}
    let contract = Contract::from_str(asset_contract)?;
    let asset = Asset::try_from(&contract)?;
    debug!(format!("asset from contract: {asset:#?}"));

    let mut allocations = vec![];
    let mut balance = 0;

    for full_utxo in &asset_utxos {
        let coins = asset.outpoint_coins(full_utxo.utxo.outpoint);
        for coin in coins.iter() {
            balance += coin.state.value;
        }
        let mut coins = coins
            .into_iter()
            .map(|coin| FullCoin {
                coin,
                terminal_derivation: full_utxo.terminal_derivation.clone(),
            })
            .collect();
        allocations.append(&mut coins);
    }

    trace!(format!("asset utxos {asset_utxos:#?}"));
    debug!(format!("allocations {allocations:#?}"));
    debug!(format!("balance {balance}"));

    if amount > balance {
        error!(format!(
            "Not enough coins. Had {balance}, but needed {amount}"
        ));
        return Err(anyhow!(
            "Not enough coins. Had {balance}, but needed {amount}"
        ));
    }

    let seal_coins: Vec<SealCoins> = allocations
        .clone()
        .into_iter()
        .map(|full_coin| SealCoins {
            amount: full_coin.coin.state.value,
            txid: full_coin.coin.seal.txid,
            vout: full_coin.coin.seal.vout,
        })
        // TODO: If we have only one asset it's okay, but if we have several it will fail. We need to allocate if we have several but if you put in 0 it will fail, so it might be an rgb-node problem
        .filter(|x| (x.amount > 0))
        .collect();
    info!("seal_coins", format!("{seal_coins:#?}"));

    // rgb20 transfer --utxo ${UTXO_SRC} --change 9900@tapret1st:${UTXO_CHANGE} ${CONSIGNMENT} 100@${TXOB} ${TRANSITION}
    let outpoints: Vec<OutPoint> = seal_coins
        .iter()
        .map(|coin| OutPoint {
            txid: coin.txid,
            vout: coin.vout,
        })
        .collect();

    // Compose consignment from provided asset contract
    let (mut consignment, consignment_details): (
        InmemConsignment<TransferConsignment>,
        ConsignmentDetails,
    ) = compose_consignment(&contract, outpoints.clone(), None).await?;

    info!(format!("Parse blinded UTXO: {blinded_utxo}"));
    let utxob = match blinded_utxo.parse() {
        Ok(utxob) => utxob,
        Err(err) => return Err(anyhow!("Error parsing supplied blinded utxo: {err}")),
    };

    // rust-rgb20 -> bin/rgb20 -> Command::Transfer
    let beneficiaries = vec![UtxobValue {
        value: amount,
        seal_confidential: utxob,
    }];
    debug!("Map beneficiaries");
    let beneficiaries = beneficiaries
        .into_iter()
        .map(|v| (v.seal_confidential.into(), amount))
        .collect();

    info!(format!("Beneficiaries: {beneficiaries:#?}"));

    debug!("Coin selection - Largest First Coin");
    let mut change: Vec<(AssignedState<_>, u64)> = vec![];
    let mut inputs = vec![];
    let mut remainder = amount;

    for full_coin in allocations {
        let descriptor = format!(
            "{}:{} {}",
            full_coin.coin.seal.txid, full_coin.coin.seal.vout, full_coin.terminal_derivation
        );
        debug!(format!(
            "Parsing InputDescriptor from outpoint: {descriptor}"
        ));
        let input_descriptor = match InputDescriptor::from_str(&descriptor) {
            Ok(desc) => desc,
            Err(err) => return Err(anyhow!("Error parsing input_descriptor: {err}")),
        };
        debug!(format!(
            "InputDescriptor successfully parsed: {input_descriptor:#?}"
        ));

        if full_coin.coin.state.value >= remainder {
            debug!("Large coins");
            // TODO: Change output must not be cloned, it needs to be a separate UTXO
            change.push((
                full_coin.coin.clone(),
                full_coin.coin.state.value - remainder,
            )); // Change
            inputs.push(input_descriptor);
            debug!(format!("Coin: {:#?}", full_coin.coin));
            debug!(format!(
                "Amount: {} - Remainder: {remainder}",
                full_coin.coin.state.value
            ));
            break;
        } else {
            debug!("Whole coins");
            change.push((full_coin.coin.clone(), full_coin.coin.state.value)); // Spend entire coin
            remainder -= full_coin.coin.state.value;
            inputs.push(input_descriptor);
            debug!(format!("Coin: {:#?}", full_coin.coin));
            debug!(format!(
                "Amount: {} - Remainder: {remainder}",
                full_coin.coin.state.value
            ));
        }
    }

    debug!(format!("Change: {change:#?}"));
    debug!(format!("Inputs: {inputs:#?}"));

    // Find an output that isn't being used as change
    let change_outputs: Vec<&LocalUtxo> = asset_utxos
        .iter()
        .map(|asset_utxo| &asset_utxo.utxo)
        .filter(|asset_utxo| {
            !change.iter().any(|(coin, _)| {
                coin.seal.txid == asset_utxo.outpoint.txid
                    && coin.seal.vout == asset_utxo.outpoint.vout
            })
        })
        .collect();

    trace!(format!("Candidate change outputs: {change_outputs:#?}"));

    // If there's no free outputs, the user needs to run fund vault again.
    if change_outputs.is_empty() {
        error!("No free outputs, the user needs to run fund vault again");
        return Err(anyhow!(
            "No free outputs, the user needs to run fund vault again"
        ));
    }
    let change_output = change_outputs.get(0).unwrap();
    debug!(format!("Selected change output: {change_output:#?}"));

    let change: BTreeMap<RevealedSeal, u64> = change
        .iter()
        .map(|(_coin, remainder)| AllocatedValue {
            value: *remainder,
            seal: ExplicitSeal {
                method: CloseMethod::TapretFirst,
                txid: Some(change_output.outpoint.txid),
                vout: change_output.outpoint.vout,
            },
        })
        .map(|v| (v.into_revealed_seal(), v.value))
        .collect();

    let outpoints: BTreeSet<OutPoint> = outpoints.into_iter().collect();

    info!("Creating state transition for asset transfer");
    debug!(format!("Outpoints: {outpoints:#?}"));
    debug!(format!("Beneficiaries: {beneficiaries:#?}"));
    debug!(format!("Change allocated values: {change:#?}"));

    let transition = match asset.transfer(outpoints.clone(), beneficiaries, change.clone()) {
        Ok(t) => t,
        Err(err) => {
            error!(format!(
                "Error creating state transition for asset transfer: {err}",
            ));
            return Err(anyhow!(
                "Error creating state transition for asset transfer"
            ));
        }
    };

    info!("Successfully created transition");
    debug!(format!("Transition: {transition:#?}"));

    // descriptor-wallet -> btc-cold -> construct
    // btc-cold construct --input "${UTXO_SRC} /0/0" --allow-tapret-path 1 ${WALLET} ${PSBT} ${FEE}
    let txid_set: BTreeSet<_> = inputs.iter().map(|input| input.outpoint.txid).collect();
    debug!(format!("txid set: {txid_set:?}"));

    let url = BITCOIN_ELECTRUM_API.read().await;
    let electrum_client = Client::new(&url)?;
    debug!(format!("Electrum client connected to {url}"));

    let tx_map = electrum_client
        .batch_transaction_get(&txid_set)?
        .into_iter()
        .map(|tx| (tx.txid(), tx))
        .collect::<BTreeMap<_, _>>();

    info!("Re-scanned network");

    let outputs = vec![]; // TODO: not sure if this is correct
    let allow_tapret_path = DfsPath::from_str("1")?;

    // format BDK descriptor for RGB
    let re = Regex::new(r"\(\[([0-9a-f]+)/(.+)](.+?)/")?;
    let cap = re.captures(bdk_rgb_assets_descriptor_xpub).unwrap();
    let rgb_assets_descriptor = format!("tr(m=[{}]/{}=[{}]/*/*)", &cap[1], &cap[2], &cap[3]);
    let rgb_assets_descriptor = rgb_assets_descriptor.replace('\'', "h");

    debug!(format!(
        "Creating descriptor wallet from RGB Tokens Descriptor: {rgb_assets_descriptor}"
    ));
    let descriptor = match Descriptor::from_str(&rgb_assets_descriptor) {
        Ok(d) => d,
        Err(err) => {
            error!(format!(
                "Error creating descriptor wallet from RGB Tokens Descriptor: {err}",
            ));
            return Err(anyhow!(
                "Error creating descriptor wallet from RGB Tokens Descriptor"
            ));
        }
    };
    // let fee = 143; // 1 sat/vByte for permanently spending dust TX
    let fee = 500; // donate entire fee to miners for OpRet TODO: different behavior for TapRet

    debug!("Constructing PSBT with...");
    debug!(format!("outputs: {outputs:?}"));
    debug!(format!("allow_tapret_path: {allow_tapret_path:?}"));
    debug!(format!("descriptor: {descriptor:#?}"));
    debug!(format!("fee: {fee:?}"));

    let mut psbt = match Psbt::construct(
        &descriptor,
        &inputs,
        &outputs,
        0_u16,
        fee,
        Some(&allow_tapret_path),
        &tx_map,
    ) {
        Ok(p) => p,
        Err(err) => {
            error!(format!(
                "Error constructing PSBT from RGB Tokens Descriptor: {err}",
            ));
            return Err(anyhow!(
                "Error constructing PSBT from RGB Tokens Descriptor"
            ));
        }
    };

    debug!(format!("PSBT successfully constructed: {psbt:#?}"));

    psbt.fallback_locktime = Some(LockTime::from_str("none")?);
    debug!(format!("Locktime set: {:#?}", psbt.fallback_locktime));

    // Embed information about the contract into the PSBT
    psbt.set_rgb_contract(contract.clone())?;
    debug!("RGB contract successfully set on PSBT");

    // Embed information about the state transition into the PSBT
    // rgb-cli -n testnet transfer combine ${CONTRACT_ID} ${TRANSITION} ${PSBT} ${UTXO_SRC}
    // rgb-node -> cli/command -> TransferCommand::Combine
    let node_id = transition.node_id();
    debug!(format!("Using Node ID: {node_id}"));
    psbt.push_rgb_transition(transition)?;
    info!("Pushed RGB state transition onto PSBT");

    let contract_id = consignment_details.contract_id;
    debug!(format!("Using contract_id: {contract_id}"));

    for input in &mut psbt.inputs {
        debug!(format!("Input: {input:#?}"));
        if outpoints.contains(&input.previous_outpoint) {
            debug!(format!(
                "Input contains previous outpoint: {}",
                input.previous_outpoint
            ));
            debug!(format!(
                "Setting RGB consumer on input for contract id: {contract_id} and node id: {node_id}"
            ));
            input.set_rgb_consumer(contract_id, node_id)?;
            debug!("RGB consumer successfully set on input");
        }
    }

    info!("Mapping outpoints on PSBT");
    debug!(format!("Mapping outpoints on PSBT: {psbt}"));
    let outpoints: BTreeSet<_> = psbt
        .inputs
        .iter()
        .map(|input| input.previous_outpoint)
        .collect();
    info!("Getting outpoint state map");
    debug!(format!("Outpoints: {outpoints:#?}"));
    let state_map = outpoint_state(&outpoints, &consignment_details)?;
    debug!(format!("Outpoint state map: {state_map:#?}"));

    for (cid, outpoint_map) in state_map {
        if cid == contract_id {
            continue;
        }
        // Embed information about the contract into the PSBT
        let (contract, _) = compose_consignment(&contract, vec![], None).await?;
        psbt.set_rgb_contract(contract)?;
        debug!("RGB contract successfully set on PSBT");
        let blank_bundle = TransitionBundle::blank(&outpoint_map, &bmap! {})?;
        for (transition, indexes) in blank_bundle.revealed_iter() {
            debug!(format!("Pushing RGB transition: {transition:#?}"));
            psbt.push_rgb_transition(transition.clone())?;
            for no in indexes {
                debug!(format!(
                    "Setting RGB consumer for contract id: {cid} and node_id: {}",
                    transition.node_id()
                ));
                psbt.inputs[*no as usize].set_rgb_consumer(cid, transition.node_id())?;
            }
        }
    }

    debug!(format!(
        "PSBT with state transition: {}",
        base64::encode(&psbt.serialize())
    ));

    // Process all state transitions under all contracts which are present in PSBT and prepare information about them which will be used in LNPBP4 commitments.
    // rgb psbt bundle ${PSBT}
    // rgb-std -> bin/rgb -> Command::Psbt -> PsbtCommand::Bundle
    let count = psbt.rgb_bundle_to_lnpbp4()?;
    info!(format!("Total {count} bundles converted"));

    // Analyze
    for contract_id in psbt.rgb_contract_ids() {
        info!(format!("- contract_id: {contract_id}"));
        if let Some(contract) = psbt.rgb_contract(contract_id)? {
            info!(format!("  - source: {contract}"));
        } else {
            info!("  - warning: contract source is absent");
        }
        info!("  - transitions:");
        for node_id in psbt.rgb_node_ids(contract_id) {
            if let Some(transition) = psbt.rgb_transition(node_id)? {
                info!(format!("    - {}", transition.strict_serialize()?.to_hex()));
            } else {
                info!("    - warning: transition is absent");
            }
        }
        info!("  - used in:");
        for (node_id, vin) in psbt.rgb_contract_consumers(contract_id)? {
            info!(format!("    - input: {vin}"));
            info!(format!("      node_id: {node_id}"));
        }
    }

    // Finalize the consignment by adding the anchor information to it referencing the txid.
    // rgb-cli -n testnet transfer finalize --endseal ${TXOB} ${PSBT} ${CONSIGNMENT} --send
    // rgb-node -> bucketd/processor -> finalize_transfer

    info!(format!("Finalizing transfer for {}...", contract_id));

    // 1. Pack LNPBP-4 and anchor information.
    info!("1. Pack LNPBP-4 and anchor information.");
    let mut bundles = psbt.rgb_bundles()?;
    info!(format!("Found {} bundles", bundles.len()));
    debug!(format!("Bundles: {bundles:#?}"));

    let anchor = Anchor::commit(&mut psbt)?;
    debug!(format!("Anchor: {anchor:#?}"));

    // 2. Extract contract-related state transition from PSBT and put it into consignment.
    info!("2. Extract contract-related state transition from PSBT and put it into consignment.");
    trace!(format!(
        "Extracting bundle with contract id: {contract_id:?}"
    ));
    let bundle = bundles.remove(&contract_id).unwrap();
    let bundle_id = bundle.bundle_id();
    consignment.push_anchored_bundle(anchor.to_merkle_proof(contract_id)?, bundle)?;

    // 3. Add seal endpoints.
    info!("3. Add seal endpoints.");
    let endseals = vec![SealEndpoint::try_from(utxob)?];
    for endseal in endseals {
        consignment.push_seal_endpoint(bundle_id, endseal);
    }

    // 4. Conceal all the state not related to the transfer.
    info!("4. Conceal all the state not related to the transfer.");
    // TODO: Conceal all the amounts except the last transition
    // TODO: Conceal all seals outside of the paths from the endpoint to genesis

    // 5. Construct and store disclosure for the blank transfers.
    info!("5. Construct and store disclosure for the blank transfers.");
    let txid = anchor.txid;
    let disclosure = Disclosure::with(anchor, bundles, None);

    debug!(format!("txid: {txid}"));
    debug!(format!("disclosure: {disclosure:#?}"));

    // Finalize, sign & publish the witness transaction
    info!("Finalize, sign & publish the witness transaction...");
    debug!(format!(
        "Finalized PSBT to be signed (base64): {}",
        base64::encode(&psbt.serialize())
    ));
    debug!(format!(
        "Finalized PSBT to be signed (hex): {}",
        hex::encode(psbt.serialize())
    ));
    debug!(format!(
        "RGB assets descriptor from BDK {bdk_rgb_assets_descriptor_xpub}"
    ));
    debug!(format!(
        "RGB assets descriptor formatted for RGB {rgb_assets_descriptor}"
    ));

    // btc-hot sign ${PSBT} ${DIR}/testnet
    // btc-cold finalize --publish testnet ${PSBT}
    // (This is done by the client methods that call this method)

    let change = change
        .into_iter()
        .map(|(s, a)| SealCoins {
            amount: a,
            txid: s.txid.unwrap(),
            vout: s.vout,
        })
        .collect();

    Ok(TransferAssetsNativeResponse {
        consignment,
        psbt: psbt.into(),
        disclosure,
        change,
        previous_utxo: seal_coins,
        new_utxo: blinded_utxo.to_owned(),
    })
}
