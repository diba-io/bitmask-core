use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

#[cfg(not(target_arch = "wasm32"))]
use amplify::Wrapper;
use anyhow::{anyhow, Result};
use bitcoin::{OutPoint, Txid};
use bp::seals::txout::CloseMethod;
use commit_verify::{
    lnpbp4::{self, MerkleBlock},
    CommitConceal,
};
use electrum_client::{Client, ConfigBuilder};
use rgb_core::{
    schema::OwnedRightType, Anchor, Assignment, Extension, OwnedRights, PedersenStrategy,
    TypedAssignments,
};
use rgb_std::{
    seal::Revealed, BundleId, Consignment, ConsignmentId, ConsignmentType, Contract, ContractId,
    ContractState, ContractStateMap, Disclosure, Genesis, InmemConsignment, Node, NodeId, Schema,
    SchemaId, SealEndpoint, Transition, TransitionBundle, Validator, Validity,
};
use storm::{chunk::ChunkIdExt, ChunkId};
use strict_encoding::{StrictDecode, StrictEncode};

use crate::{
    data::constants::{BITCOIN_ELECTRUM_API, ELECTRUM_TIMEOUT},
    debug, error, info, trace, warn,
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

// rgb-node - rpc/src/reveal
#[derive(From, PartialEq, Eq, Debug, Clone, StrictEncode, StrictDecode)]
pub struct Reveal {
    /// Outpoint blinding factor (generated when the utxo blinded was created)
    pub blinding_factor: u64,

    /// Locally-controlled outpoint (specified when the utxo blinded was created)
    pub outpoint: OutPoint,

    /// method (specified when the utxo blinded was created)
    pub close_method: CloseMethod,
}

impl std::fmt::Display for Reveal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}@{}#{}",
            self.close_method, self.outpoint, self.blinding_factor
        )
    }
}

/// Parses a blinding factor.
fn parse_blind(s: &str) -> Result<u64, ParseRevealError> {
    s.parse().map_err(ParseRevealError::BlindingFactor)
}

impl ::core::str::FromStr for Reveal {
    type Err = ParseRevealError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 9 + 19 + 1 + 64 + 1 + 10
        if s.len() > 97 {
            return Err(ParseRevealError::TooLong);
        }
        let find_method = s.find('@');
        if find_method.is_none() {
            return Err(ParseRevealError::Format);
        }

        let colon_method = find_method.unwrap();
        if colon_method == 0 || colon_method == s.len() - 1 {
            return Err(ParseRevealError::Format);
        }

        let find_blind = s.find('#');
        if find_blind.is_none() {
            return Err(ParseRevealError::Format);
        }

        let colon_blind = find_blind.unwrap();
        if colon_blind == 0 || colon_blind == s.len() - 1 {
            return Err(ParseRevealError::Format);
        }

        Ok(Reveal {
            close_method: match CloseMethod::from_str(&s[..colon_method]) {
                Ok(it) => it,
                Err(_) => return Err(ParseRevealError::CloseMethod),
            },
            outpoint: match OutPoint::from_str(&s[colon_method + 1..colon_blind]) {
                Ok(it) => it,
                Err(_) => return Err(ParseRevealError::Outpoint),
            },
            blinding_factor: parse_blind(&s[colon_blind + 1..])?,
        })
    }
}

/// An error in parsing an OutPoint.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParseRevealError {
    /// Error in outpoint part.
    CloseMethod,
    /// Error in outpoint part.
    Outpoint,
    /// Error in blinding factor part.
    BlindingFactor(::core::num::ParseIntError),
    /// Error in general format.
    Format,
    /// Size exceeds max.
    TooLong,
}

impl std::fmt::Display for ParseRevealError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ParseRevealError::CloseMethod => write!(f, "error parsing CloseMethod"),
            ParseRevealError::Outpoint => write!(f, "error parsing OutPoint"),
            ParseRevealError::BlindingFactor(ref e) => {
                write!(f, "error parsing blinding_factor: {e}")
            }
            ParseRevealError::Format => {
                write!(f, "Reveal not in <blind_factor>@<txid>:<vout> format")
            }
            ParseRevealError::TooLong => write!(f, "reveal should be at most 95 digits"),
        }
    }
}

impl ::std::error::Error for ParseRevealError {
    fn cause(&self) -> Option<&dyn ::std::error::Error> {
        match *self {
            ParseRevealError::BlindingFactor(ref e) => Some(e),
            _ => None,
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct ConsignmentDetails {
    pub transitions: BTreeMap<NodeId, Transition>,
    pub transition_witness: BTreeMap<NodeId, Txid>,
    pub anchors: BTreeMap<Txid, Anchor<MerkleBlock>>,
    pub bundles: BTreeMap<ChunkId, TransitionBundle>,
    pub contract_transitions: BTreeMap<ChunkId, BTreeSet<NodeId>>,
    pub outpoints: BTreeMap<ChunkId, BTreeSet<NodeId>>,
    pub node_contracts: BTreeMap<NodeId, ContractId>,
    pub extensions: BTreeMap<NodeId, Extension>,
    pub contracts: BTreeMap<ContractId, ContractState>,
    pub contract_id: ContractId,
    pub consignment_id: ConsignmentId,
    pub schema_id: SchemaId,
    pub schema: Schema,
    pub schemata: BTreeMap<SchemaId, Schema>,
    pub genesis: Genesis,
    pub root_schema_id: Option<SchemaId>,
    pub root_schema: Option<Schema>,
}

// rgb-node -> bucketd/processor -> process_consignment
async fn process_consignment<C: ConsignmentType>(
    consignment: &InmemConsignment<C>,
    force: bool,
    reveal: Option<Reveal>,
) -> Result<ConsignmentDetails> {
    let mut transitions: BTreeMap<NodeId, Transition> = Default::default();
    let mut transition_witness: BTreeMap<NodeId, Txid> = Default::default();
    let mut anchors: BTreeMap<Txid, Anchor<MerkleBlock>> = Default::default();
    let mut bundles: BTreeMap<ChunkId, TransitionBundle> = Default::default();
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

    let electrum_config = ConfigBuilder::new()
        .timeout(Some(ELECTRUM_TIMEOUT))?
        .build();
    let electrum_client = Client::from_config(&BITCOIN_ELECTRUM_API.read().await, electrum_config)?;

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
    // trace!(format!("Schema: {schema:#?}"));
    schemata.insert(schema_id, schema.clone());
    if let Some(root_schema) = root_schema.clone() {
        debug!(format!("Root schema: {root_schema:#?}"));
        schemata.insert(root_schema.schema_id(), root_schema);
    }

    if let Some(Reveal {
        blinding_factor,
        outpoint,
        close_method,
    }) = reveal
    {
        let reveal_outpoint = Revealed {
            method: close_method,
            blinding: blinding_factor,
            txid: Some(outpoint.txid),
            vout: outpoint.vout,
        };

        let concealed_seals = consignment
            .endpoints()
            .filter(|&&(_, seal)| reveal_outpoint.to_concealed_seal() == seal.commit_conceal())
            .clone();

        if concealed_seals.count() == 0 {
            error!(
                "The provided outpoint and blinding factors does not match with outpoint from \
                 the consignment"
            );
            Err(anyhow!(
                "The provided outpoint and blinding factors does not match with outpoint from \
            the consignment"
            ))?
        }
    };

    let genesis = consignment.genesis();
    info!("Indexing genesis");
    debug!(format!("Genesis: {genesis:#?}"));

    for seal in genesis.revealed_seals().unwrap_or_default() {
        debug!(format!("Adding outpoint for seal {seal}"));
        let index_id = ChunkId::with_fixed_fragments(
            seal.txid
                .expect("genesis with vout-based seal which passed schema validation"),
            seal.vout,
        );
        debug!(format!("index id: {index_id}"));
        let success = outpoints
            .entry(index_id)
            .or_default()
            .insert(NodeId::from_inner(contract_id.into_inner()));
        debug!(format!(
            "insertion into outpoints BTreeMap success: {success}"
        ));
    }
    debug!("Storing contract self-reference");
    node_contracts.insert(NodeId::from_inner(contract_id.into_inner()), contract_id);

    let anchored_bundles = consignment.anchored_bundles();
    trace!(format!(
        "Processing anchored bundles: {anchored_bundles:#?}"
    ));

    for (anchor, bundle) in anchored_bundles {
        let bundle_id = bundle.bundle_id();
        let witness_txid = anchor.txid;
        info!(format!(
            "Processing anchored bundle {bundle_id} for txid {witness_txid}"
        ));
        debug!(format!("Anchor: {anchor:?}"));
        debug!(format!("Bundle: {bundle:?}"));
        let anchor = anchor.to_merkle_block(contract_id, bundle_id.into())?;
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

            // TODO: refactoring this and move to rgb-core
            let new_transition = match reveal {
                Some(Reveal {
                    blinding_factor,
                    outpoint,
                    close_method,
                }) => {
                    let reveal_outpoint = Revealed {
                        method: close_method,
                        blinding: blinding_factor,
                        txid: Some(outpoint.txid),
                        vout: outpoint.vout,
                    };

                    let mut owned_rights: BTreeMap<OwnedRightType, TypedAssignments> = bmap! {};
                    for (owned_type, assignments) in transition.owned_rights().iter() {
                        let outpoints = assignments.to_value_assignments();

                        let mut revealed_assignment: Vec<Assignment<PedersenStrategy>> = empty!();

                        for out in outpoints {
                            if out.commit_conceal().to_confidential_seal()
                                != reveal_outpoint.to_concealed_seal()
                            {
                                revealed_assignment.push(out);
                            } else {
                                let accept = match out.as_revealed_state() {
                                    Some(seal) => Assignment::Revealed {
                                        seal: reveal_outpoint,
                                        state: *seal,
                                    },
                                    _ => out,
                                };
                                revealed_assignment.push(accept);
                            }
                        }

                        owned_rights
                            .insert(*owned_type, TypedAssignments::Value(revealed_assignment));
                    }

                    let tmp: Transition = Transition::with(
                        transition.transition_type(),
                        transition.metadata().clone(),
                        transition.parent_public_rights().clone(),
                        OwnedRights::from(owned_rights),
                        transition.public_rights().clone(),
                        transition.parent_owned_rights().clone(),
                    );

                    tmp
                }
                _ => transition.to_owned(),
            };

            trace!(format!("State transition: {new_transition:?}"));
            state.add_transition(witness_txid, &new_transition);

            debug!(format!("Contract state now is {state:?}"));

            debug!("Storing state transition data");
            revealed.insert(new_transition.clone(), inputs.clone());
            transitions.insert(node_id, new_transition.clone());
            transition_witness.insert(node_id, witness_txid);

            debug!("Indexing transition");
            let index_id = ChunkId::with_fixed_fragments(contract_id, transition_type);
            trace!(format!("index_id: {index_id:?}"));
            contract_transitions
                .entry(index_id)
                .or_default()
                .insert(node_id);

            node_contracts.insert(node_id, contract_id);

            for seal in new_transition.filter_revealed_seals() {
                let index_id = ChunkId::with_fixed_fragments(
                    seal.txid.expect("seal should contain revealed txid"),
                    seal.vout,
                );
                outpoints.entry(index_id).or_default().insert(node_id);
            }
        }

        let data = TransitionBundle::with(revealed, concealed)?;

        // bundles.insert(witness_txid, data);
        let chunk_id = ChunkId::with_fixed_fragments(contract_id, witness_txid);
        trace!(format!("Insert bundle with chunk_id: {chunk_id:?}"));
        bundles.insert(chunk_id, data);
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
        node_ids: impl IntoIterator<Item = NodeId> + Debug,
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

        trace!(format!("Iterating over node ids: {node_ids:#?}"));

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
                let chunk_id = ChunkId::with_fixed_fragments(*contract_id, *witness_txid);
                trace!(format!("Retrieving chunk_id from bundle: {chunk_id:?}"));
                let bundle: TransitionBundle = bundles.get(&chunk_id).unwrap().to_owned();
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
pub async fn compose_consignment<T: ConsignmentType>(
    contract: &Contract,
    utxos: Vec<OutPoint>,
    reveal: Option<Reveal>,
) -> Result<(InmemConsignment<T>, ConsignmentDetails)> {
    info!("Composing consignment");
    let consignment_details = process_consignment(contract, true, reveal).await?;
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
pub fn outpoint_state(
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

#[allow(dead_code)]
fn process_disclosure(
    consignment_details: &mut ConsignmentDetails,
    disclosure: Disclosure,
) -> Result<()> {
    for (_anchor_id, (anchor, bundle_map)) in disclosure.anchored_bundles().iter() {
        for (contract_id, bundle) in bundle_map {
            let state: &mut ContractState = consignment_details
                .contracts
                .get_mut(contract_id)
                .expect("state absent");
            trace!(format!("Starting with contract state {state:?}"));

            let bundle_id = bundle.bundle_id();
            let witness_txid = anchor.txid;
            debug!(format!(
                "Processing anchored bundle {bundle_id} for txid {witness_txid}",
            ));
            trace!(format!("Anchor: {anchor:?}"));
            trace!(format!("Bundle: {bundle:?}"));
            consignment_details
                .anchors
                .insert(anchor.txid, anchor.to_owned());
            let concealed: BTreeMap<NodeId, BTreeSet<u16>> = bundle
                .concealed_iter()
                .map(|(id, set)| (*id, set.clone()))
                .collect();
            let mut revealed: BTreeMap<Transition, BTreeSet<u16>> = bmap!();
            for (transition, inputs) in bundle.revealed_iter() {
                let node_id = transition.node_id();
                let transition_type = transition.transition_type();
                debug!(format!("Processing state transition {node_id}"));
                trace!(format!("State transition: {transition:?}"));

                state.add_transition(witness_txid, transition);
                trace!(format!("Contract state now is {state:?}"));

                trace!("Storing state transition data");
                revealed.insert(transition.clone(), inputs.to_owned());
                consignment_details
                    .transitions
                    .insert(node_id, transition.to_owned());
                consignment_details
                    .transition_witness
                    .insert(node_id, witness_txid.to_owned());

                trace!("Indexing transition");
                let index_id = ChunkId::with_fixed_fragments(*contract_id, transition_type);
                consignment_details
                    .contract_transitions
                    .entry(index_id)
                    .or_default()
                    .insert(node_id);
                consignment_details
                    .node_contracts
                    .insert(node_id, contract_id.to_owned());

                for seal in transition.filter_revealed_seals() {
                    let index_id = ChunkId::with_fixed_fragments(
                        seal.txid.expect("seal should contain revealed txid"),
                        seal.vout,
                    );
                    consignment_details
                        .outpoints
                        .entry(index_id)
                        .or_default()
                        .insert(node_id);
                }
            }
            let data = TransitionBundle::with(revealed, concealed)?;
            let chunk_id = ChunkId::with_fixed_fragments(*contract_id, witness_txid);
            consignment_details.bundles.insert(chunk_id, data);
        }
    }

    Ok(())
}
