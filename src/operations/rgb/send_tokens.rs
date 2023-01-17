use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use anyhow::{anyhow, Result};
use bitcoin::{consensus::deserialize, psbt::PartiallySignedTransaction, OutPoint};
use bitcoin_scripts::taproot::DfsPath;
use bp::seals::txout::{CloseMethod, ExplicitSeal};
use electrum_client::{Client, ConfigBuilder, ElectrumApi};
use miniscript_crate::Descriptor;
use psbt::{serialize::Serialize, Psbt};
use regex::Regex;
use rgb20::Asset;
use rgb_core::{Anchor, IntoRevealedSeal};
use rgb_std::{
    fungible::allocation::{AllocatedValue, UtxobValue},
    psbt::{RgbExt, RgbInExt},
    Contract, Disclosure, InmemConsignment, Node as RgbNode, SealEndpoint, TransferConsignment,
};
use strict_encoding::strict_serialize;
use wallet::descriptors::InputDescriptor;

use crate::{
    data::{
        constants::{BITCOIN_ELECTRUM_API, ELECTRUM_TIMEOUT},
        structs::{FullCoin, SealCoins},
    },
    debug, error, info,
    rgb::shared::{compose_consignment, ConsignmentDetails},
    util, TransfersRequest, TransfersResponse,
};

pub async fn transfer_asset(request: TransfersRequest) -> Result<TransfersResponse> {
    debug!(format!("total transfers: {}", request.transfers.len()));

    let mut asset_utxos = vec![];
    let mut transitions = vec![];
    let mut contracts = vec![];
    let mut transfers = vec![];
    let mut transaction_info = vec![];

    for transfer in request.transfers {
        // rgb-cli transfer compose ${CONTRACT_ID} ${UTXO_SRC} ${CONSIGNMENT}
        let contract = Contract::from_str(&transfer.asset_contract)?;
        let asset = Asset::try_from(&contract)?;
        debug!(format!("asset from contract: {asset:#?}"));

        let mut balance = 0;
        let mut allocations = vec![];

        let asset_utxo = transfer.asset_utxo.clone();
        let coins = asset.outpoint_coins(OutPoint::from_str(&asset_utxo.outpoint)?);
        for coin in coins.clone() {
            balance += coin.state.value
        }

        let mut coins = coins
            .into_iter()
            .map(|coin| FullCoin {
                coin,
                terminal_derivation: asset_utxo.terminal_derivation.clone(),
            })
            .collect();
        allocations.append(&mut coins);

        if transfer.asset_amount > balance {
            error!(format!(
                "The contract {0} bot enough coins. Had {balance}, but needed {1}",
                contract.contract_id(),
                transfer.asset_amount
            ));
            return Err(anyhow!(
                "The contract {0} bot enough coins. Had {balance}, but needed {1}",
                contract.contract_id(),
                transfer.asset_amount
            ));
        }

        let seal_coins: Vec<SealCoins> = allocations
            .into_iter()
            .map(|full_coin| SealCoins {
                amount: full_coin.coin.state.value,
                txid: full_coin.coin.seal.txid,
                vout: full_coin.coin.seal.vout,
            })
            // TODO: If we have only one asset it's okay, but if we have several it will fail. We need to allocate if we have several but if you put in 0 it will fail, so it might be an rgb-node problem
            .filter(|x| (x.amount > 0))
            .collect();

        // rgb20 transfer --utxo ${UTXO_SRC} --change 9900@tapret1st:${UTXO_CHANGE} ${CONSIGNMENT} 100@${TXOB} ${TRANSITION}
        let outpoints: Vec<OutPoint> = seal_coins
            .iter()
            .map(|coin| OutPoint {
                txid: coin.txid,
                vout: coin.vout,
            })
            .collect();

        let mut remainder = balance;
        let beneficiaries: BTreeMap<SealEndpoint, u64> = transfer
            .beneficiaries
            .into_iter()
            .map(|b| UtxobValue::from_str(&b).expect("Beneficiary must be a valid blinded utxo"))
            .into_iter()
            .map(|v| {
                remainder -= v.value;
                (v.seal_confidential.into(), v.value)
            })
            .collect();

        let change = OutPoint::from_str(&transfer.change_utxo)?;
        let changes = vec![AllocatedValue {
            value: remainder,
            seal: ExplicitSeal {
                method: CloseMethod::TapretFirst,
                txid: Some(change.txid),
                vout: change.vout,
            },
        }];

        let changes_info = changes.clone();

        let changes = changes
            .into_iter()
            .map(|v| (v.into_revealed_seal(), v.value))
            .collect();

        let inputs = outpoints.clone().into_iter().collect();
        let transition = match asset.transfer(inputs, beneficiaries.clone(), changes) {
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

        let (consignment, _): (InmemConsignment<TransferConsignment>, ConsignmentDetails) =
            compose_consignment(&contract, outpoints, None).await?;

        asset_utxos.push(asset_utxo);
        contracts.push(contract.clone());
        transitions.push((contract.contract_id(), transition));
        let consignment_serialize =
            strict_serialize(&consignment).expect("Consignment information must be valid");
        let consignment_serialize = util::bech32m_zip_encode("rgbc", &consignment_serialize)
            .expect("Strict encoded information must be a valid consignment");
        transfers.push((consignment, beneficiaries.into_keys().collect::<Vec<_>>()));
        transaction_info.push((
            contract.contract_id().to_string(),
            changes_info,
            seal_coins,
            consignment_serialize.clone(),
        ));
    }

    // descriptor-wallet -> btc-cold -> construct
    // btc-cold construct --input "${UTXO_SRC} /0/0" --allow-tapret-path 1 ${WALLET} ${PSBT} ${FEE}
    let txid_set: BTreeSet<_> = asset_utxos
        .iter()
        .map(|input| OutPoint::from_str(&input.outpoint).unwrap().txid)
        .collect();
    debug!(format!("txid set: {txid_set:?}"));

    let fee = 500;
    let allow_tapret_path = DfsPath::from_str("1")?;
    let input_descs: Vec<InputDescriptor> = asset_utxos
        .clone()
        .into_iter()
        .map(|full| {
            let descriptor = format!("{} {}", full.outpoint, full.terminal_derivation,);
            InputDescriptor::from_str(&descriptor).expect("Error parsing input_descriptor")
        })
        .collect();

    let url = BITCOIN_ELECTRUM_API.read().await;
    let electrum_config = ConfigBuilder::new()
        .timeout(Some(ELECTRUM_TIMEOUT))?
        .build();
    let electrum_client = Client::from_config(&url, electrum_config)?;
    debug!(format!("Electrum client connected to {url}"));

    let tx_map = electrum_client
        .batch_transaction_get(&txid_set)?
        .into_iter()
        .map(|tx| (tx.txid(), tx))
        .collect::<BTreeMap<_, _>>();

    // format BDK descriptor for RGB
    let re = Regex::new(r"\(\[([0-9a-f]+)/(.+)](.+?)/")?;
    let cap = re.captures(&request.descriptor_xpub).unwrap();
    let rgb_assets_descriptor = format!("tr(m=[{}]/{}=[{}]/*/*)", &cap[1], &cap[2], &cap[3]);
    let rgb_assets_descriptor = rgb_assets_descriptor.replace('\'', "h");

    let descriptor = Descriptor::from_str(&rgb_assets_descriptor)?;

    let mut psbt = match Psbt::construct(
        &descriptor,
        &input_descs,
        vec![],
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

    debug!(format!("Locktime set: {:#?}", psbt.fallback_locktime));

    // Embed information about the contract into the PSBT
    for contract in contracts {
        psbt.set_rgb_contract(contract.clone())?;
    }

    debug!("RGB contract successfully set on PSBT");

    // Embed information about the state transition into the PSBT
    // rgb-cli -n testnet transfer combine ${CONTRACT_ID} ${TRANSITION} ${PSBT} ${UTXO_SRC}
    for (contract_id, transition) in transitions {
        let node_id = transition.node_id();
        debug!(format!("Using Node ID: {node_id}"));
        psbt.push_rgb_transition(transition.clone())?;
        info!("Pushed RGB state transition onto PSBT");

        debug!(format!("Using contract_id: {contract_id}"));

        for input in &mut psbt.inputs {
            debug!(format!("Input: {input:#?}"));
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

    // rgb psbt bundle ${PSBT}
    // rgb-std -> bin/rgb -> Command::Psbt -> PsbtCommand::Bundle
    let _count = psbt.rgb_bundle_to_lnpbp4()?;

    // rgb-cli -n testnet transfer finalize --endseal ${TXOB} ${PSBT} ${CONSIGNMENT} --send
    // 1. Pack LNPBP-4 and anchor information.
    info!("1. Pack LNPBP-4 and anchor information.");
    let mut bundles = psbt.rgb_bundles()?;
    info!(format!("Found {} bundles", bundles.len()));
    debug!(format!("Bundles: {bundles:#?}"));

    let anchor = Anchor::commit(&mut psbt)?;
    debug!(format!("Anchor: {anchor:#?}"));

    let mut consignments = vec![];
    for (state_transfer, seal_endpoints) in &transfers {
        let mut consignment = state_transfer.clone();
        let contract_id = consignment.contract_id();
        debug!(format!("Finalizing transfer for {contract_id}"));

        // 2. Extract contract-related state transition from PSBT and put it
        //    into consignment.
        let bundle = bundles
            .remove(&contract_id)
            .expect("Contract must be inside in transition bundle");
        let bundle_id = bundle.bundle_id();
        consignment.push_anchored_bundle(anchor.to_merkle_proof(contract_id)?, bundle)?;

        // 3. Add seal endpoints.
        let endseals = seal_endpoints.clone();
        for endseal in endseals {
            consignment.push_seal_endpoint(bundle_id, endseal);
        }

        consignments.push((consignment, seal_endpoints.to_owned()));
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
        "RGB assets descriptor from BDK {0}",
        request.descriptor_xpub
    ));
    debug!(format!(
        "RGB assets descriptor formatted for RGB {rgb_assets_descriptor}"
    ));

    info!("Successfully created assets PSBT");
    let psbt = base64::decode(&base64::encode(&psbt.serialize()))?;
    let psbt: PartiallySignedTransaction = deserialize(&psbt)?;

    Ok(TransfersResponse {
        psbt,
        origin: asset_utxos,
        disclosure,
        transfers: consignments,
        transaction_info,
    })
}
