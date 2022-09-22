use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use amplify::{hex::ToHex, Wrapper};
use anyhow::{anyhow, Result};
use bdk::{database::AnyDatabase, descriptor::Descriptor, LocalUtxo, Wallet};
use bitcoin::{psbt::serialize::Serialize, OutPoint, Transaction, Txid};
use bp::seals::txout::{CloseMethod, ExplicitSeal};
use commit_verify::lnpbp4::{self, MerkleBlock};
use electrum_client::{Client, ElectrumApi};
use regex::Regex;
use rgb20::Asset;
use rgb_core::{Anchor, Extension, IntoRevealedSeal};
use rgb_std::{
    blank::BlankBundle,
    fungible::allocation::{AllocatedValue, UtxobValue},
    psbt::{RgbExt, RgbInExt},
    AssignedState, BundleId, Consignment, ConsignmentId, ConsignmentType, Contract, ContractId,
    ContractState, ContractStateMap, Disclosure, Genesis, InmemConsignment, Node, NodeId, Schema,
    SchemaId, SealEndpoint, Transition, TransitionBundle, Validator, Validity,
};
use storm::{ChunkId, ChunkIdExt};
use strict_encoding::{StrictDecode, StrictEncode};
use wallet::{
    descriptors::InputDescriptor, locks::LockTime, psbt::Psbt, scripts::taproot::DfsPath,
};

use crate::{
    data::{
        constants::BITCOIN_ELECTRUM_API,
        structs::{SealCoins, TransferResponse},
    },
    debug, error, info,
    operations::bitcoin::{sign_psbt, synchronize_wallet},
    trace, warn,
};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, StrictEncode, StrictDecode)]
pub enum OutpointFilter {
    All,
    Only(BTreeSet<OutPoint>),
}

impl OutpointFilter {
    pub fn includes(&self, outpoint: OutPoint) -> bool {
        match self {
            OutpointFilter::All => true,
            OutpointFilter::Only(set) => set.contains(&outpoint),
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct ConsignmentDetails {
    transitions: BTreeMap<NodeId, Transition>,
    transition_witness: BTreeMap<NodeId, Txid>,
    anchors: BTreeMap<Txid, Anchor<MerkleBlock>>,
    bundles: BTreeMap<Txid, TransitionBundle>,
    contract_transitions: BTreeMap<ChunkId, BTreeSet<NodeId>>,
    outpoints: BTreeMap<ChunkId, BTreeSet<NodeId>>,
    node_contracts: BTreeMap<NodeId, ContractId>,
    extensions: BTreeMap<NodeId, Extension>,
    contracts: BTreeMap<ContractId, ContractState>,
    contract_id: ContractId,
    consignment_id: ConsignmentId,
    schema_id: SchemaId,
    schema: Schema,
    schemata: BTreeMap<SchemaId, Schema>,
    genesis: Genesis,
    root_schema_id: Option<SchemaId>,
    root_schema: Option<Schema>,
}

// rgb-node -> bucketd/processor -> process_consignment
async fn process_consignment<C: ConsignmentType>(
    consignment: &InmemConsignment<C>,
    force: bool,
) -> Result<ConsignmentDetails> {
    let mut transitions: BTreeMap<NodeId, Transition> = Default::default();
    let mut transition_witness: BTreeMap<NodeId, Txid> = Default::default();
    let mut anchors: BTreeMap<Txid, Anchor<MerkleBlock>> = Default::default();
    let mut bundles: BTreeMap<Txid, TransitionBundle> = Default::default();
    let mut contract_transitions: BTreeMap<ChunkId, BTreeSet<NodeId>> = Default::default();
    let mut outpoints: BTreeMap<ChunkId, BTreeSet<NodeId>> = Default::default();
    let mut node_contracts: BTreeMap<NodeId, ContractId> = Default::default();
    let mut extensions: BTreeMap<NodeId, Extension> = Default::default();
    let mut contracts: BTreeMap<ContractId, ContractState> = Default::default();
    let mut schemata: BTreeMap<SchemaId, Schema> = Default::default();

    let contract_id = consignment.contract_id();
    let consignment_id = consignment.id();
    let schema = consignment.schema();
    let schema_id = consignment.schema_id();
    let genesis = consignment.genesis();
    let root_schema = consignment.root_schema().cloned();
    let root_schema_id = consignment.root_schema_id();

    info!(format!(
        "Registering consignment {consignment_id} for contract {contract_id}"
    ));

    let mut state = ContractState::with(schema_id, root_schema_id, contract_id, genesis);
    debug!(format!("Starting with contract state {state:#?}"));

    info!(format!(
        "Validating consignment {consignment_id} for contract {contract_id}"
    ));
    let electrum_client = Client::new(&BITCOIN_ELECTRUM_API.read().await)?;
    let status = Validator::validate(consignment, &electrum_client);
    info!(format!(
        "Consignment validation result is {}",
        status.validity()
    ));

    match status.validity() {
        Validity::Valid => {
            info!("Consignment is fully valid");
        }
        Validity::ValidExceptEndpoints if force => {
            warn!("Forcing import of consignment with non-mined transactions");
        }
        Validity::UnresolvedTransactions | Validity::ValidExceptEndpoints => {
            error!(format!(
                "Some of consignment-related transactions were not found: {status:?}",
            ));
            return Err(anyhow!(status.to_string()));
        }
        Validity::Invalid => {
            error!(format!("Invalid consignment: {status:?}"));
            return Err(anyhow!(status.to_string()));
        }
    }

    info!(format!(
        "Storing consignment {consignment_id} into database"
    ));
    trace!(format!("Schema: {schema:#?}"));
    schemata.insert(schema_id, schema.clone());
    if let Some(root_schema) = root_schema.clone() {
        debug!(format!("Root schema: {root_schema:#?}"));
        schemata.insert(root_schema.schema_id(), root_schema);
    }

    let genesis = consignment.genesis();
    info!("Indexing genesis");
    debug!(format!("Genesis: {genesis:#?}"));

    for seal in genesis.revealed_seals().unwrap_or_default() {
        debug!(format!("Adding outpoint for seal {seal}"));
        let index_id = ChunkId::with_fixed_fragments(seal.txid, seal.vout);
        debug!(format!("index id: {index_id}"));
        let success = outpoints
            .entry(index_id)
            .or_insert(BTreeSet::new())
            .insert(NodeId::from_inner(contract_id.into_inner()));
        debug!(format!(
            "insertion into outpoints BTreeMap success: {success}"
        ));
    }
    debug!("Storing contract self-reference");
    node_contracts.insert(NodeId::from_inner(contract_id.into_inner()), contract_id);

    for (anchor, bundle) in consignment.anchored_bundles() {
        let bundle_id = bundle.bundle_id();
        let witness_txid = anchor.txid;
        info!(format!(
            "Processing anchored bundle {bundle_id} for txid {witness_txid}"
        ));
        debug!(format!("Anchor: {anchor:?}"));
        debug!(format!("Bundle: {bundle:?}"));
        let anchor = anchor
            .to_merkle_block(contract_id, bundle_id.into())
            .expect("broken anchor data");
        info!(format!("Restored anchor id is {}", anchor.anchor_id()));
        debug!(format!("Restored anchor: {anchor:?}"));
        anchors.insert(anchor.txid, anchor);

        let concealed: BTreeMap<NodeId, BTreeSet<u16>> = bundle
            .concealed_iter()
            .map(|(id, set)| (*id, set.clone()))
            .collect();
        let mut revealed: BTreeMap<Transition, BTreeSet<u16>> = bmap!();

        for (transition, inputs) in bundle.revealed_iter() {
            let node_id = transition.node_id();
            let transition_type = transition.transition_type();
            info!(format!("Processing state transition {node_id}"));
            debug!(format!("State transition: {transition:?}"));

            state.add_transition(witness_txid, transition);
            debug!(format!("Contract state now is {state:?}"));

            debug!("Storing state transition data");
            revealed.insert(transition.clone(), inputs.clone());
            transitions.insert(node_id, transition.clone());
            transition_witness.insert(node_id, witness_txid);

            debug!("Indexing transition");
            let index_id = ChunkId::with_fixed_fragments(contract_id, transition_type);
            contract_transitions
                .entry(index_id)
                .or_insert(BTreeSet::new())
                .insert(node_id);

            node_contracts.insert(node_id, contract_id);

            for seal in transition.revealed_seals().unwrap_or_default() {
                let index_id = ChunkId::with_fixed_fragments(
                    seal.txid.expect("seal should contain revealed txid"),
                    seal.vout,
                );
                outpoints
                    .entry(index_id)
                    .or_insert(BTreeSet::new())
                    .insert(node_id);
            }
        }

        // let mut bundle_data = BTreeMap::new();
        // for (node_id, inputs) in data {
        //     bundle_data.insert(node_id, inputs.clone());
        // }

        let data = TransitionBundle::with(revealed, concealed)
            .expect("enough data should be available to create bundle");

        bundles.insert(witness_txid, data);
    }
    for extension in consignment.state_extensions() {
        let node_id = extension.node_id();
        info!(format!("Processing state extension {node_id}"));
        debug!(format!("State transition: {extension:?}"));

        state.add_extension(extension);
        debug!(format!("Contract state now is {state:?}"));

        node_contracts.insert(node_id, contract_id);

        extensions.insert(node_id, extension.clone());
        // We do not store seal outpoint here - or will have to store it into a separate
        // database Extension rights are always closed seals, since the extension
        // can get into the history only through closing by a state transition
    }

    info!(format!("Storing contract state for {contract_id}"));
    debug!(format!("Final contract state is {state:#?}"));
    contracts.insert(contract_id, state);

    info!(format!(
        "Consignment processing complete for {consignment_id}"
    ));

    Ok(ConsignmentDetails {
        transitions,
        transition_witness,
        anchors,
        bundles,
        contract_transitions,
        node_contracts,
        outpoints,
        extensions,
        contracts,
        contract_id,
        consignment_id,
        schema_id,
        schema: schema.to_owned(),
        schemata,
        genesis: genesis.to_owned(),
        root_schema_id,
        root_schema,
    })
}

struct Collector {
    pub anchored_bundles: BTreeMap<Txid, (Anchor<lnpbp4::MerkleProof>, TransitionBundle)>,
    pub endpoints: Vec<(BundleId, SealEndpoint)>,
    pub endpoint_inputs: Vec<NodeId>,
}

impl Collector {
    pub fn new() -> Self {
        Collector {
            anchored_bundles: empty![],
            endpoints: vec![],
            endpoint_inputs: vec![],
        }
    }

    pub fn process(
        &mut self,
        consignment_details: &ConsignmentDetails,
        node_ids: impl IntoIterator<Item = NodeId>,
        outpoint_filter: &OutpointFilter,
    ) -> Result<()> {
        let ConsignmentDetails {
            transitions,
            transition_witness,
            anchors,
            bundles,
            contract_id,
            ..
        } = consignment_details;

        for transition_id in node_ids {
            if transition_id.to_vec() == contract_id.to_vec() {
                continue;
            }
            let transition: &Transition = transitions.get(&transition_id).unwrap();

            let witness_txid: &Txid = transition_witness.get(&transition_id).unwrap();

            let bundle = if let Some((_, bundle)) = self.anchored_bundles.get_mut(witness_txid) {
                bundle
            } else {
                let anchor: &Anchor<lnpbp4::MerkleBlock> = anchors.get(witness_txid).unwrap();
                let bundle: TransitionBundle = bundles.get(witness_txid).unwrap().to_owned();
                let anchor = anchor.to_merkle_proof(*contract_id)?;
                self.anchored_bundles
                    .insert(*witness_txid, (anchor, bundle));
                &mut self
                    .anchored_bundles
                    .get_mut(witness_txid)
                    .expect("stdlib is broken")
                    .1
            };

            let bundle_id = bundle.bundle_id();
            for (_, assignments) in transition.owned_rights().iter() {
                for seal in assignments.filter_revealed_seals() {
                    let txid = seal.txid.unwrap_or(*witness_txid);
                    let outpoint = OutPoint::new(txid, seal.vout);
                    let seal_endpoint = SealEndpoint::from(seal);
                    if outpoint_filter.includes(outpoint) {
                        self.endpoints.push((bundle_id, seal_endpoint));
                        self.endpoint_inputs.extend(
                            transition
                                .parent_outputs()
                                .into_iter()
                                .map(|out| out.node_id),
                        );
                    }
                }
            }

            bundle.reveal_transition(transition.to_owned())?;
        }

        Ok(())
    }

    pub fn iterate(mut self, consignment_details: &ConsignmentDetails) -> Result<Self> {
        // Collect all transitions between endpoints and genesis independently from their type
        loop {
            let node_ids = self.endpoint_inputs;
            self.endpoint_inputs = vec![];
            self.process(consignment_details, node_ids, &OutpointFilter::All)?;
            if self.endpoint_inputs.is_empty() {
                break;
            }
        }
        Ok(self)
    }

    pub fn into_consignment<T: ConsignmentType>(
        self,
        schema: Schema,
        root_schema: Option<Schema>,
        genesis: Genesis,
    ) -> Result<InmemConsignment<T>> {
        let anchored_bundles = self
            .anchored_bundles
            .into_values()
            .collect::<Vec<_>>()
            .try_into()?;

        Ok(InmemConsignment::<T>::with(
            schema,
            root_schema,
            genesis,
            self.endpoints,
            anchored_bundles,
            empty!(),
        ))
    }
}

// rgb-node -> bucketd/processor -> compose_consignment
async fn compose_consignment<T: ConsignmentType>(
    contract: &Contract,
    utxos: Vec<OutPoint>,
) -> Result<(InmemConsignment<T>, ConsignmentDetails)> {
    info!("Composing consignment");
    let consignment_details = process_consignment(contract, true).await?;
    debug!("Consignment successfully processed");
    let details = consignment_details.clone();

    let mut collector = Collector::new();

    debug!("Processing consignment into a consignment collector");
    for transition_type in details.schema.transitions.keys() {
        let chunk_id = ChunkId::with_fixed_fragments(details.contract_id, *transition_type);
        debug!(format!("ChunkId: {chunk_id}"));
        let node_ids: BTreeSet<NodeId> = details
            .contract_transitions
            .get(&chunk_id)
            .unwrap_or(&BTreeSet::new())
            .to_owned();
        debug!(format!("node_ids: {node_ids:#?}"));
        let outpoints: BTreeSet<OutPoint> = utxos.clone().into_iter().collect();
        debug!(format!("outpoints: {outpoints:#?}"));
        let filter = OutpointFilter::Only(outpoints);
        collector.process(&details, node_ids, &filter)?;
    }
    debug!("Consignment successfully processed into collector");

    collector = collector.iterate(&details)?;

    let consignment = collector.into_consignment(
        consignment_details.schema,
        consignment_details.root_schema,
        consignment_details.genesis,
    )?;

    debug!("Consignment collector successfully processed into consignment");

    Ok((consignment, details))
}

// rgb-node -> bucketd/processor -> outpoint_state
fn outpoint_state(
    outpoints: &BTreeSet<OutPoint>,
    consignment_details: &ConsignmentDetails,
) -> Result<ContractStateMap> {
    let mut res: ContractStateMap = bmap! {};

    if outpoints.is_empty() {
        error!("Outpoints provided to outpoint_state is empty");
    }

    let indexes: BTreeSet<ChunkId> = outpoints
        .iter()
        .map(|outpoint| ChunkId::with_fixed_fragments(outpoint.txid, outpoint.vout))
        .collect();

    for index in &indexes {
        let set: BTreeSet<NodeId> = match consignment_details.outpoints.get(index) {
            Some(set) => set.clone(),
            None => bset! {},
        };
        for node_id in set {
            let contract_id: &ContractId =
                consignment_details.node_contracts.get(&node_id).unwrap();

            let state: &ContractState = consignment_details.contracts.get(contract_id).unwrap();

            let map = if outpoints.is_empty() {
                state.all_outpoint_state()
            } else {
                state.filter_outpoint_state(outpoints)
            };

            res.insert(*contract_id, map);
        }
    }

    Ok(res)
}

pub async fn transfer_asset(
    blinded_utxo: &str,
    amount: u64,
    asset_contract: &str, // rgb1...
    assets_wallet: &Wallet<AnyDatabase>,
    bdk_rgb_assets_descriptor_xpub: &str,
) -> Result<(ConsignmentDetails, Transaction, TransferResponse)> {
    // TODO: the pure bdk part before signing and sending must be the method compose_btc_psbt, the rgb part must be compose_rgb_psbt and then sign and send the corresponding method
    // BDK
    info!("sync wallet");
    synchronize_wallet(assets_wallet).await?;
    info!("wallet synced");
    let asset_utxos = assets_wallet.list_unspent()?;

    debug!(format!("asset_contract: {asset_contract}"));

    // RGB
    // rgb-cli -n testnet transfer compose ${CONTRACT_ID} ${UTXO_SRC} ${CONSIGNMENT}
    let contract = Contract::from_str(asset_contract)?;
    debug!(format!("parsed contract: {contract}"));
    let asset = Asset::try_from(&contract)?;
    debug!(format!("asset from contract: {asset:#?}"));

    let mut allocations = vec![];
    let mut balance = 0;

    for utxo in &asset_utxos {
        let mut coins = asset.outpoint_coins(utxo.outpoint);
        for coin in coins.iter() {
            balance += coin.state.value;
        }
        allocations.append(&mut coins);
    }

    trace!(format!("asset utxos {:#?}", &asset_utxos));
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
        .map(|coin| SealCoins {
            amount: coin.state.value,
            txid: coin.seal.txid,
            vout: coin.seal.vout,
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
    let (mut consignment, consignment_details) =
        compose_consignment(&contract, outpoints.clone()).await?;

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

    for coin in allocations {
        let descriptor = format!("{}:{} /0/0", coin.seal.txid, coin.seal.vout);
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

        if coin.state.value >= remainder {
            debug!("Large coins");
            // TODO: Change output must not be cloned, it needs to be a separate UTXO
            change.push((coin.clone(), coin.state.value - remainder)); // Change
            inputs.push(input_descriptor);
            debug!(format!("Coin: {coin:#?}"));
            debug!(format!(
                "Amount: {} - Remainder: {remainder}",
                coin.state.value
            ));
            break;
        } else {
            debug!("Whole coins");
            change.push((coin.clone(), coin.state.value)); // Spend entire coin
            remainder -= coin.state.value;
            inputs.push(input_descriptor);
            debug!(format!("Coin: {coin:#?}"));
            debug!(format!(
                "Amount: {} - Remainder: {remainder}",
                coin.state.value
            ));
        }
    }

    debug!(format!("Change: {change:#?}"));
    debug!(format!("Inputs: {inputs:#?}"));

    // Find an output that isn't being used as change
    let change_outputs: Vec<&LocalUtxo> = asset_utxos
        .iter()
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
        error!("no free outputs, the user needs to run fund vault again");
        return Err(anyhow!(
            "no free outputs, the user needs to run fund vault again"
        ));
    }
    let change_output = change_outputs.get(0).unwrap();
    debug!(format!("Selected change output: {change_output:#?}"));

    let change = change
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

    let transition = match asset.transfer(outpoints.clone(), beneficiaries, change) {
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
    let re = Regex::new(r"\(\[([0-9a-f]+)/(.+)](.+?)/").unwrap();
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
    let fee = 500;

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
    psbt.set_rgb_contract(contract)?;
    debug!("RGB contract successfully set on PSBT");

    // Embed information about the state transition into the PSBT
    // rgb-cli -n testnet transfer combine ${CONTRACT_ID} ${TRANSITION} ${PSBT} ${UTXO_SRC}
    // rgb-node -> cli/command -> TransferCommand::Combine
    let node_id = transition.node_id();
    debug!(format!("Using Node ID: {node_id}"));
    psbt.push_rgb_transition(transition)?;
    info!("Pushed state RGB state transition onto PSBT");

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
        hex::encode(&psbt.serialize())
    ));
    debug!(format!(
        "RGB assets descriptor from BDK {bdk_rgb_assets_descriptor_xpub}"
    ));
    debug!(format!(
        "RGB assets descriptor formatted for RGB {rgb_assets_descriptor}"
    ));

    // btc-hot sign ${PSBT} ${DIR}/testnet
    // btc-cold finalize --publish testnet ${PSBT}
    let tx = sign_psbt(assets_wallet, psbt.into()).await?;

    let txid = tx.txid().to_string();

    Ok((
        process_consignment(&consignment, true).await?,
        tx,
        TransferResponse {
            consignment: consignment.to_string(),
            disclosure: serde_json::to_string(&disclosure)?,
            txid,
        },
    ))
}

pub async fn compose_btc_psbt(_) -> _ {
    todo!()
}

pub async fn compose_rgb_psbt(
    asset: &str,
    inputs: Vec<OutPoint>,
    psbt: &mut Psbt,
    new: BTreeSet<_>,
    receiver: &str,
    change: std::collections::BTreeMap<rgb_core::seal::Revealed, u64>,
    amount: u64,
) -> _ {
    todo!()
}

pub async fn sign_and_send_psbt(_) -> _ {
    todo!()
}
