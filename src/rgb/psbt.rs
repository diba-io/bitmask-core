use std::{collections::BTreeSet, str::FromStr};

use amplify::hex::{FromHex, ToHex};
use bdk::FeeRate;
use bitcoin::{
    blockdata::opcodes,
    hashes::{sha256, Hash},
    psbt::TapTree,
    schnorr::TapTweak,
    secp256k1::SECP256K1,
    util::{
        bip32::{self, Fingerprint},
        taproot::{LeafVersion, TapBranchHash, TapLeafHash, TaprootBuilder, TaprootBuilderError},
    },
    TxIn, TxOut, Txid,
};
use bitcoin::{EcdsaSighashType, OutPoint, Script, XOnlyPublicKey};
// TODO: Incompatible versions between RGB and Descriptor Wallet
use bitcoin_30::{secp256k1::SECP256K1 as SECP256K1_30, ScriptBuf};
use bitcoin_blockchain::locks::SeqNo;
use bitcoin_scripts::PubkeyScript;
use bp::{dbc::tapret::TapretCommitment, Outpoint, TapScript, Vout};
use commit_verify::{mpc::Commitment, CommitVerify};
use miniscript_crate::{Descriptor, ForEachKey, ToPublicKey};
use psbt::{ProprietaryKey, ProprietaryKeyType, PsbtVersion};
use rgb::{
    psbt::{
        DbcPsbtError, TapretKeyError, PSBT_OUT_TAPRET_COMMITMENT, PSBT_OUT_TAPRET_HOST,
        PSBT_TAPRET_PREFIX,
    },
    DeriveInfo, MiningStatus, Resolver, RgbDescr, RgbWallet, TerminalPath, Utxo,
};
use wallet::{
    descriptors::{self, derive::DeriveDescriptor, InputDescriptor},
    hd::{DerivationAccount, DerivationSubpath, DeriveError, UnhardenedIndex},
    onchain::{ResolveTx, TxResolverError},
    psbt::{ProprietaryKeyDescriptor, ProprietaryKeyError, ProprietaryKeyLocation, Psbt},
};

use crate::{
    debug, info,
    rgb::{constants::RGB_PSBT_TAPRET, structs::AddressAmount},
    structs::{AssetType, PsbtInputRequest, PsbtSigHashRequest},
};

use crate::rgb::structs::AddressFormatParseError;

// Base weight of a Txin, not counting the weight needed for satisfying it.
// prev_txid (32 bytes) + prev_vout (4 bytes) + sequence (4 bytes)
const TXIN_BASE_WEIGHT: usize = (32 + 4 + 4) * 4;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum CreatePsbtError {
    /// At least 1 input to create PSBT file
    EmptyInputs,
    /// Invalid terminal path. '{0}'
    WrongTerminal(bip32::Error),
    /// Invalid descriptor. '{0}'
    WrongDescriptor(String),
    /// Invalid taptweak. '{0}'
    WrongTapTweak(String),
    /// Invalid input. '{0}'
    WrongInput(PsbtInputError),
    /// Invalid output address. '{0:?}'
    WrongAddress(AddressFormatParseError),
    /// PSBT file cannot be created. '{0}'
    Incomplete(String),
    /// Invalid PSBT proprietry key. '{0}'
    WrongMetadata(ProprietaryKeyError),
    /// The PSBT is invalid (Unexpected behavior).
    Inconclusive,
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum PsbtInputError {
    WrongTerminal,
    WrongDerivation(String),
    /// The transaction is invalid: {0}
    WrongPrevOut(String),
    /// The input tweak is invalid: {0}
    WrongInputTweak(String),
    /// The watcher tweak is invalid: {0}
    WrongWatcherTweak(String),
    /// The script is invalid: {0}
    WrongScript(String),
    /// The Input PSBT is invalid (Unexpected behavior).
    Inconclusive,
}

#[allow(clippy::too_many_arguments)]
pub fn create_psbt(
    psbt_inputs: Vec<PsbtInputRequest>,
    psbt_outputs: Vec<String>,
    bitcoin_fee: u64,
    terminal_change: Option<String>,
    wallet: Option<RgbWallet>,
    tx_resolver: &impl ResolveTx,
    options: PsbtNewOptions,
) -> Result<(Psbt, String), CreatePsbtError> {
    if psbt_inputs.is_empty() {
        return Err(CreatePsbtError::EmptyInputs);
    }

    let mut inputs = vec![];

    // Define "Universal" Descriptor
    let psbt_input = psbt_inputs[0].clone();
    let wildcard_terminal = "/*/*";
    let mut descriptor_pub = psbt_input.descriptor.to_string();
    for contract_type in [
        AssetType::RGB20,
        AssetType::RGB21,
        AssetType::Contract,
        AssetType::Bitcoin,
        AssetType::Change,
    ] {
        let contract_index = contract_type as u32;
        let terminal_step = format!("/{contract_index}/*");
        if descriptor_pub.contains(&terminal_step) {
            descriptor_pub = descriptor_pub.replace(&terminal_step, wildcard_terminal);
            break;
        }
    }

    let global_descriptor: &Descriptor<DerivationAccount> = &Descriptor::from_str(&descriptor_pub)
        .map_err(|op| CreatePsbtError::WrongDescriptor(op.to_string()))?;

    // Define Input Descriptors
    for psbt_input in psbt_inputs {
        let new_input = InputDescriptor::resolve_psbt_input(
            psbt_input,
            global_descriptor.clone(),
            wallet.clone(),
            tx_resolver,
        )
        .map_err(CreatePsbtError::WrongInput)?;

        inputs.push(new_input);
    }

    let bitcoin_addresses: Vec<AddressAmount> = psbt_outputs
        .into_iter()
        .map(|btc| {
            AddressAmount::from_str(btc.as_str())
                .map_err(CreatePsbtError::WrongAddress)
                .expect("")
        })
        .collect();

    let outputs: Vec<(PubkeyScript, u64)> = bitcoin_addresses
        .into_iter()
        .map(|AddressAmount { address, amount }| (address.script_pubkey().into(), amount))
        .collect();

    // Change Terminal Derivation
    let mut change_index = DerivationSubpath::new();
    if let Some(terminal_change) = terminal_change {
        change_index = terminal_change
            .parse::<DerivationSubpath<UnhardenedIndex>>()
            .map_err(CreatePsbtError::WrongTerminal)?;
    }

    let psbt = Psbt::new(
        global_descriptor,
        &inputs,
        &outputs,
        change_index.to_vec(),
        bitcoin_fee,
        tx_resolver,
        options,
    )
    .map_err(|op| CreatePsbtError::Incomplete(op.to_string()))?;

    Ok((psbt, change_index.to_string()))
}

pub fn set_tapret_output(psbt: Psbt, pos: u16) -> Result<Psbt, CreatePsbtError> {
    let mut psbt = psbt;

    if pos > 0 {
        psbt.outputs.swap(0, pos.into());
    }

    // Define Tapret Proprierties
    let proprietary_keys = vec![ProprietaryKeyDescriptor {
        location: ProprietaryKeyLocation::Output(0),
        ty: ProprietaryKeyType {
            prefix: RGB_PSBT_TAPRET.to_owned(),
            subtype: 0,
        },
        key: None,
        value: None,
    }];

    for key in proprietary_keys {
        match key.location {
            ProprietaryKeyLocation::Input(pos) if pos as usize >= psbt.inputs.len() => {
                return Err(CreatePsbtError::WrongMetadata(
                    ProprietaryKeyError::InputOutOfRange(pos, psbt.inputs.len()),
                ))
            }
            ProprietaryKeyLocation::Output(pos) if pos as usize >= psbt.outputs.len() => {
                return Err(CreatePsbtError::WrongMetadata(
                    ProprietaryKeyError::OutputOutOfRange(pos, psbt.inputs.len()),
                ))
            }
            ProprietaryKeyLocation::Global => {
                psbt.proprietary.insert(
                    key.clone().into(),
                    key.value.as_ref().cloned().unwrap_or_default(),
                );
            }
            ProprietaryKeyLocation::Input(pos) => {
                psbt.inputs[pos as usize].proprietary.insert(
                    key.clone().into(),
                    key.value.as_ref().cloned().unwrap_or_default(),
                );
            }
            ProprietaryKeyLocation::Output(pos) => {
                psbt.outputs[pos as usize].proprietary.insert(
                    key.clone().into(),
                    key.value.as_ref().cloned().unwrap_or_default(),
                );
            }
        }
    }

    Ok(psbt)
}

pub fn extract_commit(psbt: Psbt) -> Result<(Outpoint, Vec<u8>), DbcPsbtError> {
    let (index, output) = psbt
        .outputs
        .iter()
        .enumerate()
        .find(|(_, output)| {
            output.proprietary.contains_key(&ProprietaryKey {
                prefix: PSBT_TAPRET_PREFIX.to_vec(),
                subtype: PSBT_OUT_TAPRET_HOST,
                key: vec![],
            })
        })
        .ok_or(DbcPsbtError::NoHostOutput)
        .expect("none of the outputs is market as a commitment host");

    let commit_vec = output.proprietary.get(&ProprietaryKey {
        prefix: PSBT_TAPRET_PREFIX.to_vec(),
        subtype: PSBT_OUT_TAPRET_COMMITMENT,
        key: vec![],
    });

    match commit_vec {
        Some(commit) => {
            let txid = bp::Txid::from_hex(&psbt.to_txid().to_hex()).expect("invalid outpoint");
            let vout = Vout::from_str(&index.to_string()).expect("invalid vout");
            Ok((Outpoint::new(txid, vout), commit.to_owned()))
        }
        _ => Err(DbcPsbtError::TapretKey(TapretKeyError::InvalidProof)),
    }
}

pub fn save_tap_commit_str(outpoint: &str, commit: &str, terminal: &str, wallet: &mut RgbWallet) {
    let outpoint = OutPoint::from_str(outpoint).expect("invalid outpoint parse");

    let outpoint = Outpoint::new(
        bp::Txid::from_str(&outpoint.txid.to_hex()).expect("invalid outpoint parse"),
        outpoint.vout,
    );

    let commit = Vec::<u8>::from_hex(commit).expect("invalid tap commit parse");

    save_tap_commit(outpoint, commit, terminal, wallet);
}

pub fn save_tap_commit(
    outpoint: Outpoint,
    commit: Vec<u8>,
    terminal: &str,
    wallet: &mut RgbWallet,
) {
    let descr = wallet.descr.clone();
    let RgbDescr::Tapret(mut tapret) = descr;
    let derive: Vec<&str> = terminal.split('/').filter(|s| !s.is_empty()).collect();
    let app = derive[0];
    let index = derive[1];

    let terminal = TerminalPath {
        app: app.parse().expect("invalid derive app"),
        index: index.parse().expect("invalid derive index"),
    };

    let mpc = Commitment::from_str(&commit.to_hex()).expect("invalid tapret");
    let tap_commit = TapretCommitment::with(mpc, 0);
    if let Some(taprets) = tapret.taprets.get(&terminal) {
        let mut current_taprets = taprets.clone();
        current_taprets.insert(tap_commit.clone());

        tapret.taprets.insert(terminal, current_taprets.clone());
    } else {
        tapret.taprets.insert(terminal, bset! {tap_commit.clone()});
    }

    wallet.utxos.insert(Utxo {
        amount: 0,
        outpoint,
        status: MiningStatus::Mempool,
        derivation: DeriveInfo::with(terminal.app, terminal.index, Some(tap_commit.clone())),
    });
    wallet.descr = RgbDescr::Tapret(tapret);
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum EstimateFeeError {
    /// Invalid address. {0:?}
    WrongAddress(AddressFormatParseError),
    /// The Input PSBT is invalid. {0}
    WrongPsbtInput(PsbtInputError),
    /// Invalid descriptor. '{0}'
    WrongDescriptor(String),
    /// Invalid terminal change. '{0}'
    WrongTerminal(String),
    /// The Pre-PSBT is invalid. {0}
    PreBuildFail(String),
    /// Insufficient funds (expected: {input} sats / current: {output} sats)
    Inflation {
        /// Amount spent: input amounts
        input: u64,

        /// Amount sent: sum of output value + transaction fee
        output: u64,
    },
}

#[allow(clippy::too_many_arguments)]
pub fn estimate_fee_tx<T>(
    assets_inputs: Vec<PsbtInputRequest>,
    bitcoin_inputs: Vec<PsbtInputRequest>,
    bitcoin_changes: Vec<String>,
    fee_rate: f32,
    wallet: &mut RgbWallet,
    amount_change: Option<u64>,
    terminal_change: Option<String>,
    resolver: &mut T,
) -> Result<(u64, u64), EstimateFeeError>
where
    T: ResolveTx + Resolver,
{
    info!("Estimate Fee (RGB)");
    // Define Feerate
    let fee_rate = FeeRate::from_sat_per_vb(fee_rate);
    let mut psbt_inputs = assets_inputs.clone();
    psbt_inputs.extend(bitcoin_inputs);

    // Define "Universal" Descriptor
    let psbt_input = assets_inputs[0].clone();
    let wildcard_terminal = "/*/*";
    let mut descriptor_pub = psbt_input.descriptor.to_string();
    for contract_type in [
        AssetType::RGB20,
        AssetType::RGB21,
        AssetType::Contract,
        AssetType::Bitcoin,
        AssetType::Change,
    ] {
        let contract_index = contract_type as u32;
        let terminal_step = format!("/{contract_index}/*");
        if descriptor_pub.contains(&terminal_step) {
            descriptor_pub = descriptor_pub.replace(&terminal_step, wildcard_terminal);
            break;
        }
    }

    // Total Inputs
    let mut psbt_inputs_total = 0;
    for psbt_input in psbt_inputs.clone() {
        let outpoint = OutPoint::from_str(&psbt_input.utxo).expect("invalid outpoint");
        if let Ok(tx) = resolver.resolve_tx(outpoint.txid) {
            if let Some(vout) = tx.output.to_vec().get(outpoint.vout as usize) {
                psbt_inputs_total = vout.value;
            }
        }
    }

    // Total Output
    let mut total_psbt_output = 0;
    let mut bitcoin_addresses: Vec<AddressAmount> = vec![];
    for bitcoin_change in bitcoin_changes {
        let recipient =
            AddressAmount::from_str(&bitcoin_change).map_err(EstimateFeeError::WrongAddress)?;
        total_psbt_output += recipient.amount;
        bitcoin_addresses.push(recipient);
    }

    let change = match psbt_inputs_total
        .checked_sub(total_psbt_output + amount_change.unwrap_or_default())
    {
        Some(change) => change,
        None => {
            return Err(EstimateFeeError::Inflation {
                input: psbt_inputs_total,
                output: total_psbt_output + amount_change.unwrap_or_default(),
            })
        }
    };

    // Pre-build PSBT
    let global_descriptor: Descriptor<DerivationAccount> = Descriptor::from_str(&descriptor_pub)
        .map_err(|op| CreatePsbtError::WrongDescriptor(op.to_string()))
        .map_err(|op| EstimateFeeError::WrongDescriptor(op.to_string()))?;

    let mut inputs = vec![];
    for psbt_input in psbt_inputs {
        let new_input = InputDescriptor::resolve_psbt_input(
            psbt_input,
            global_descriptor.clone(),
            Some(wallet.clone()),
            resolver,
        )
        .map_err(EstimateFeeError::WrongPsbtInput)?;
        inputs.push(new_input);
    }

    let outputs: Vec<(PubkeyScript, u64)> = bitcoin_addresses
        .into_iter()
        .map(|AddressAmount { address, amount }| (address.script_pubkey().into(), amount))
        .collect();

    let mut change_index = DerivationSubpath::new();
    if let Some(terminal_change) = terminal_change {
        change_index = terminal_change
            .parse::<DerivationSubpath<UnhardenedIndex>>()
            .map_err(|op| EstimateFeeError::WrongTerminal(op.to_string()))?;
    }

    let pre_psbt = Psbt::construct(
        &global_descriptor,
        &inputs,
        &outputs,
        change_index.to_vec(),
        0,
        resolver,
    )
    .map_err(|op| EstimateFeeError::PreBuildFail(op.to_string()))?;

    // Over-simplification of bdk fee calculation:
    // https://github.com/bitcoindevkit/bdk/blob/2867e88b64b4a8cf7136cc562ec61c077737a087/crates/bdk/src/wallet/mod.rs#L1009-L1131
    // https://github.com/bitcoindevkit/bdk/blob/2867e88b64b4a8cf7136cc562ec61c077737a087/crates/bdk/src/wallet/coin_selection.rs#L398-L630
    let max_w = global_descriptor
        .max_satisfaction_weight()
        .map_err(|op| EstimateFeeError::WrongDescriptor(op.to_string()))?;
    let mut fee = fee_rate.fee_wu(pre_psbt.into_unsigned_tx().weight());
    fee += fee_rate.fee_wu(TXIN_BASE_WEIGHT + max_w) * inputs.len() as u64;
    fee += fee_rate.fee_wu(2);

    // Change Amount
    let (change, fee) = match change.checked_sub(fee) {
        Some(change) => {
            debug!(format!("Change/Fee {change} ({fee})"));
            (change, fee)
        }
        None => {
            debug!(format!("No Change/Fee {change} ({fee})"));
            (0, fee)
        }
    };

    Ok((change, fee))
}

pub trait PsbtInputEx<T> {
    type Error: std::error::Error;

    fn resolve_psbt_input(
        psbt_input: PsbtInputRequest,
        descriptor: Descriptor<DerivationAccount>,
        wallet: Option<RgbWallet>,
        tx_resolver: &impl ResolveTx,
    ) -> Result<T, Self::Error>;
}

impl PsbtInputEx<InputDescriptor> for InputDescriptor {
    type Error = PsbtInputError;

    fn resolve_psbt_input(
        psbt_input: PsbtInputRequest,
        descriptor: Descriptor<DerivationAccount>,
        wallet: Option<RgbWallet>,
        tx_resolver: &impl ResolveTx,
    ) -> Result<Self, Self::Error> {
        let sighash: PsbtSigHashRequest = psbt_input.sigh_hash.unwrap_or_default();
        let outpoint: OutPoint = psbt_input.utxo.parse().expect("invalid outpoint parse");

        let mut input: InputDescriptor = InputDescriptor {
            outpoint,
            terminal: psbt_input
                .utxo_terminal
                .parse::<DerivationSubpath<UnhardenedIndex>>()
                .map_err(|_| PsbtInputError::WrongTerminal)?,
            seq_no: SeqNo::default(),
            tweak: None,
            sighash_type: EcdsaSighashType::from_consensus(sighash as u32),
        };

        // Verify TapTweak (User Input or Watcher inspect)
        if let Some(tapret) = psbt_input.tapret {
            input.tweak = Some((
                Fingerprint::default(),
                tapret
                    .parse::<sha256::Hash>()
                    .map_err(|op| PsbtInputError::WrongInputTweak(op.to_string()))?,
            ))
        } else if let Some(tweak) = complete_input_desc(
            descriptor.clone(),
            input.clone(),
            wallet.clone(),
            tx_resolver,
        )
        .map_err(|op| PsbtInputError::WrongWatcherTweak(op.to_string()))?
        {
            input.tweak = Some((
                Fingerprint::default(),
                tweak
                    .parse::<sha256::Hash>()
                    .map_err(|op| PsbtInputError::WrongWatcherTweak(op.to_string()))?,
            ))
        }

        Ok(input)
    }
}

fn complete_input_desc(
    descriptor: Descriptor<DerivationAccount>,
    input: InputDescriptor,
    wallet: Option<RgbWallet>,
    tx_resolver: &impl ResolveTx,
) -> Result<Option<String>, PsbtInputError> {
    let txid = input.outpoint.txid;
    let tx = tx_resolver
        .resolve_tx(txid)
        .map_err(|op| PsbtInputError::WrongPrevOut(op.to_string()))?;
    let prev_output = tx.output.get(input.outpoint.vout as usize).unwrap();

    let mut scripts = bmap![];
    if let Descriptor::Tr(_) = descriptor {
        let output_descriptor = DeriveDescriptor::<XOnlyPublicKey>::derive_descriptor(
            &descriptor,
            SECP256K1,
            &input.terminal,
        )
        .map_err(|op| PsbtInputError::WrongDerivation(op.to_string()))?;

        scripts.insert(output_descriptor.script_pubkey(), None);
        if let Some(walet) = wallet {
            let RgbDescr::Tapret(tapret_desc) = walet.descr;

            let idx = match input.terminal.first() {
                Some(idx) => idx,
                _ => return Err(PsbtInputError::WrongTerminal),
            };
            let contract_index =
                u32::from_str(&idx.to_string()).map_err(|_| PsbtInputError::WrongTerminal)?;

            let idx = match input.terminal.last() {
                Some(idx) => idx,
                _ => return Err(PsbtInputError::WrongTerminal),
            };
            let change_index =
                u32::from_str(&idx.to_string()).map_err(|_| PsbtInputError::WrongTerminal)?;

            let terminal = TerminalPath {
                app: contract_index,
                index: change_index,
            };

            if let Some(taprets) = tapret_desc.taprets.get(&terminal) {
                for tweak in taprets {
                    if let Descriptor::<XOnlyPublicKey>::Tr(tr_desc) = &output_descriptor {
                        let xonly = tr_desc.internal_key();

                        let tap_tweak = ScriptBuf::from_bytes(TapScript::commit(tweak).to_vec());

                        if let Ok(tap_builder) =
                            bitcoin_30::taproot::TaprootBuilder::with_capacity(1)
                                .add_leaf(0, tap_tweak)
                                .map_err(|op| PsbtInputError::WrongWatcherTweak(op.to_string()))
                        {
                            // TODO: Incompatible versions between RGB and Descriptor Wallet
                            let xonly_30 =
                                bitcoin_30::secp256k1::XOnlyPublicKey::from_str(&xonly.to_hex())
                                    .map_err(|op| {
                                        PsbtInputError::WrongWatcherTweak(op.to_string())
                                    })?;

                            let spent_info =
                                tap_builder.finalize(SECP256K1_30, xonly_30).map_err(|_| {
                                    PsbtInputError::WrongWatcherTweak("incomplete tree".to_string())
                                })?;

                            if let Some(merkle_root) = spent_info.merkle_root() {
                                let tap_script = ScriptBuf::new_v1_p2tr(
                                    SECP256K1_30,
                                    xonly_30,
                                    Some(merkle_root),
                                );

                                let spk = Script::from_str(&tap_script.as_script().to_hex())
                                    .map_err(|op| {
                                        PsbtInputError::WrongWatcherTweak(op.to_string())
                                    })?;
                                scripts.insert(spk, Some(merkle_root.to_hex()));
                            }
                        }
                    }
                }
            }
        }
    };

    if let Some((_, scp)) = scripts
        .into_iter()
        .find(|(sc, _)| sc.clone() == prev_output.script_pubkey)
    {
        Ok(scp)
    } else {
        Err(PsbtInputError::WrongScript(
            "derived scriptPubkey does not match transaction scriptPubkey".to_string(),
        ))
    }
}

pub trait PsbtEx<T> {
    type Error: std::error::Error;

    fn new<'inputs, 'outputs>(
        descriptor: &Descriptor<DerivationAccount>,
        inputs: impl IntoIterator<Item = &'inputs InputDescriptor>,
        outputs: impl IntoIterator<Item = &'outputs (PubkeyScript, u64)>,
        change_index: Vec<UnhardenedIndex>,
        fee: u64,
        tx_resolver: &impl ResolveTx,
        options: PsbtNewOptions,
    ) -> Result<T, Self::Error>;
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum PsbtConstructError {
    /// unable to construct PSBT due to one of transaction inputs is not known
    #[from]
    ResolvingTx(TxResolverError),

    /// unable to construct PSBT due to failing key derivetion derivation
    #[from]
    Derive(DeriveError),

    /// unable to construct PSBT due to spent transaction {0} not having
    /// referenced output #{1}
    OutputUnknown(Txid, u32),

    /// derived scriptPubkey `{3}` does not match transaction scriptPubkey
    /// `{2}` for {0}:{1}
    ScriptPubkeyMismatch(Txid, u32, Script, Script),

    /// one of PSBT outputs has invalid script data. {0}
    #[from]
    Miniscript(miniscript_crate::Error),

    /// taproot script tree construction error. {0}
    #[from]
    TaprootBuilderError(TaprootBuilderError),

    /// PSBT can't be constructed according to the consensus rules since
    /// it spends more ({output} sats) than the sum of its input amounts
    /// ({input} sats)
    Inflation {
        /// Amount spent: input amounts
        input: u64,

        /// Amount sent: sum of output value + transaction fee
        output: u64,
    },
}

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub struct PsbtNewOptions {
    pub set_tapret: bool,
    pub check_inflation: bool,
    pub force_inflation: u64,
}

impl Default for PsbtNewOptions {
    fn default() -> Self {
        Self {
            set_tapret: true,
            check_inflation: true,
            force_inflation: 0,
        }
    }
}

impl PsbtNewOptions {
    pub fn set_inflaction(inflaction: u64) -> Self {
        Self {
            set_tapret: true,
            check_inflation: false,
            force_inflation: inflaction,
        }
    }
}

impl PsbtEx<Psbt> for Psbt {
    type Error = PsbtConstructError;

    fn new<'inputs, 'outputs>(
        descriptor: &Descriptor<DerivationAccount>,
        inputs: impl IntoIterator<Item = &'inputs InputDescriptor>,
        outputs: impl IntoIterator<Item = &'outputs (PubkeyScript, u64)>,
        change_index: Vec<UnhardenedIndex>,
        fee: u64,
        tx_resolver: &impl ResolveTx,
        options: PsbtNewOptions,
    ) -> Result<Psbt, PsbtConstructError> {
        let mut xpub = bmap! {};
        descriptor.for_each_key(|account| {
            if let Some(key_source) = account.account_key_source() {
                xpub.insert(account.account_xpub, key_source);
            }
            true
        });

        let mut total_spent = 0u64;
        let mut psbt_inputs: Vec<psbt::Input> = vec![];

        for (index, input) in inputs.into_iter().enumerate() {
            let txid = input.outpoint.txid;
            let mut tx = tx_resolver.resolve_tx(txid)?;

            // Cut out witness data
            for inp in &mut tx.input {
                inp.witness = zero!();
            }

            let prev_output = tx
                .output
                .get(input.outpoint.vout as usize)
                .ok_or(PsbtConstructError::OutputUnknown(txid, input.outpoint.vout))?;
            let (script_pubkey, dtype, tr_descriptor, pretr_descriptor, tap_tree) = match descriptor
            {
                Descriptor::Tr(_) => {
                    let output_descriptor = DeriveDescriptor::<XOnlyPublicKey>::derive_descriptor(
                        descriptor,
                        SECP256K1,
                        &input.terminal,
                    )?;

                    if input.tweak.is_some() {
                        let mut tap_tree: Option<TapBranchHash> = None;
                        let mut tap_script = output_descriptor.script_pubkey();
                        if let Descriptor::<XOnlyPublicKey>::Tr(tr_desc) = output_descriptor.clone()
                        {
                            if let Some((_, tweak)) = input.tweak {
                                let merkel_tree =
                                    TapBranchHash::from_slice(&tweak).expect("invalid taptweak");
                                let internal_key = tr_desc.internal_key().to_x_only_pubkey();
                                let (output_key, _) =
                                    internal_key.tap_tweak(SECP256K1, Some(merkel_tree));
                                let builder = bitcoin::blockdata::script::Builder::new();

                                tap_tree = Some(merkel_tree);
                                tap_script = builder
                                    .push_opcode(opcodes::all::OP_PUSHNUM_1)
                                    .push_slice(&output_key.serialize())
                                    .into_script();
                            }
                        }
                        (
                            tap_script,
                            descriptors::CompositeDescrType::from(&output_descriptor),
                            Some(output_descriptor),
                            None,
                            tap_tree,
                        )
                    } else {
                        (
                            output_descriptor.script_pubkey(),
                            descriptors::CompositeDescrType::from(&output_descriptor),
                            Some(output_descriptor),
                            None,
                            None,
                        )
                    }
                }
                _ => {
                    let output_descriptor =
                        DeriveDescriptor::<bitcoin::PublicKey>::derive_descriptor(
                            descriptor,
                            SECP256K1,
                            &input.terminal,
                        )?;
                    (
                        output_descriptor.script_pubkey(),
                        descriptors::CompositeDescrType::from(&output_descriptor),
                        None,
                        Some(output_descriptor),
                        None,
                    )
                }
            };
            if prev_output.script_pubkey != script_pubkey {
                return Err(PsbtConstructError::ScriptPubkeyMismatch(
                    txid,
                    input.outpoint.vout,
                    prev_output.script_pubkey.clone(),
                    script_pubkey,
                ));
            }
            let mut bip32_derivation = bmap! {};
            let result = descriptor.for_each_key(|account| {
                match account.bip32_derivation(SECP256K1, &input.terminal) {
                    Ok((pubkey, key_source)) => {
                        bip32_derivation.insert(pubkey, key_source);
                        true
                    }
                    Err(_) => false,
                }
            });
            if !result {
                return Err(DeriveError::DerivePatternMismatch.into());
            }

            total_spent += prev_output.value;

            let tx_in = TxIn {
                previous_output: input.outpoint,
                ..Default::default()
            };

            let mut psbt_input =
                psbt::Input::new(index, tx_in).expect("Error when contruct PSBT Input");
            psbt_input.sequence_number = Some(input.seq_no);
            psbt_input.bip32_derivation = bip32_derivation;
            psbt_input.sighash_type = Some(input.sighash_type.into());

            if dtype.is_segwit() {
                psbt_input.witness_utxo = Some(prev_output.clone());
            }
            // This is required even in case of segwit outputs, since at least Ledger Nano X
            // do not trust just `non_witness_utxo` data.
            psbt_input.non_witness_utxo = Some(tx.clone());

            if let Some(Descriptor::<XOnlyPublicKey>::Tr(tr)) = tr_descriptor {
                psbt_input.bip32_derivation.clear();
                psbt_input.tap_merkle_root = tr.spend_info().merkle_root();
                psbt_input.tap_merkle_root = tap_tree;
                psbt_input.tap_internal_key = Some(tr.internal_key().to_x_only_pubkey());
                let spend_info = tr.spend_info();
                psbt_input.tap_scripts = spend_info
                    .as_script_map()
                    .iter()
                    .map(|((script, leaf_ver), _)| {
                        (
                            spend_info
                                .control_block(&(script.clone(), *leaf_ver))
                                .expect("taproot scriptmap is broken"),
                            (script.clone(), *leaf_ver),
                        )
                    })
                    .collect();
                if let Some(taptree) = tr.taptree() {
                    descriptor.for_each_key(|key| {
                        let (pubkey, key_source) = key
                            .bip32_derivation(SECP256K1, &input.terminal)
                            .expect("failing on second pass of the same function");
                        let pubkey = XOnlyPublicKey::from(pubkey);
                        let mut leaves = vec![];
                        for (_, ms) in taptree.iter() {
                            for pk in ms.iter_pk() {
                                if pk == pubkey {
                                    leaves.push(TapLeafHash::from_script(
                                        &ms.encode(),
                                        LeafVersion::TapScript,
                                    ));
                                }
                            }
                        }
                        let entry = psbt_input
                            .tap_key_origins
                            .entry(pubkey.to_x_only_pubkey())
                            .or_insert((vec![], key_source));
                        entry.0.extend(leaves);
                        true
                    });
                }
                descriptor.for_each_key(|key| {
                    let (pubkey, key_source) = key
                        .bip32_derivation(SECP256K1, &input.terminal)
                        .expect("failing on second pass of the same function");
                    let pubkey = XOnlyPublicKey::from(pubkey);
                    if pubkey == *tr.internal_key() {
                        psbt_input
                            .tap_key_origins
                            .entry(pubkey.to_x_only_pubkey())
                            .or_insert((vec![], key_source));
                    }
                    true
                });
                for (leaves, _) in psbt_input.tap_key_origins.values_mut() {
                    *leaves = leaves
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                }
            } else if let Some(output_descriptor) = pretr_descriptor {
                let lock_script = output_descriptor.explicit_script()?;
                if dtype.has_redeem_script() {
                    psbt_input.redeem_script = Some(lock_script.clone().into());
                }
                if dtype.has_witness_script() {
                    psbt_input.witness_script = Some(lock_script.into());
                }
            }

            psbt_inputs.push(psbt_input);
        }

        let mut total_sent = 0u64;
        let mut psbt_outputs: Vec<_> = outputs
            .into_iter()
            .enumerate()
            .map(|(index, (script, amount))| {
                total_sent += *amount;
                let txout = TxOut {
                    value: *amount,
                    script_pubkey: script.clone().into(),
                };
                psbt::Output::new(index, txout)
            })
            .collect();

        let change = if !options.check_inflation {
            options.force_inflation
        } else {
            match total_spent.checked_sub(total_sent + fee) {
                Some(change) => change,
                None => {
                    return Err(PsbtConstructError::Inflation {
                        input: total_spent,
                        output: total_sent + fee,
                    })
                }
            }
        };

        if change > 0 {
            let change_derivation: [UnhardenedIndex; 2] =
                change_index.try_into().expect("invalid hardened index");
            let mut bip32_derivation = bmap! {};
            let bip32_derivation_fn = |account: &DerivationAccount| {
                let (pubkey, key_source) = account
                    .bip32_derivation(SECP256K1, change_derivation)
                    .expect("already tested descriptor derivation mismatch");
                bip32_derivation.insert(pubkey, key_source);
                true
            };

            let change_txout = TxOut {
                value: change,
                ..Default::default()
            };
            let mut psbt_change_output = psbt::Output::new(psbt_outputs.len(), change_txout);
            if let Descriptor::Tr(_) = descriptor {
                let change_descriptor = DeriveDescriptor::<XOnlyPublicKey>::derive_descriptor(
                    descriptor,
                    SECP256K1,
                    change_derivation,
                )?;
                let change_descriptor = match change_descriptor {
                    Descriptor::Tr(tr) => tr,
                    _ => unreachable!(),
                };

                psbt_change_output.script = change_descriptor.script_pubkey().into();
                descriptor.for_each_key(bip32_derivation_fn);

                let internal_key: XOnlyPublicKey =
                    change_descriptor.internal_key().to_x_only_pubkey();
                psbt_change_output.tap_internal_key = Some(internal_key);
                if let Some(tree) = change_descriptor.taptree() {
                    let mut builder = TaprootBuilder::new();
                    for (depth, ms) in tree.iter() {
                        builder = builder
                            .add_leaf(depth, ms.encode())
                            .expect("insane miniscript taptree");
                    }
                    psbt_change_output.tap_tree =
                        Some(TapTree::try_from(builder).expect("non-finalized TaprootBuilder"));
                }
            } else {
                let change_descriptor = DeriveDescriptor::<bitcoin::PublicKey>::derive_descriptor(
                    descriptor,
                    SECP256K1,
                    change_derivation,
                )?;
                psbt_change_output.script = change_descriptor.script_pubkey().into();

                let dtype = descriptors::CompositeDescrType::from(&change_descriptor);
                descriptor.for_each_key(bip32_derivation_fn);

                let lock_script = change_descriptor.explicit_script()?;
                if dtype.has_redeem_script() {
                    psbt_change_output.redeem_script = Some(lock_script.clone().into());
                }
                if dtype.has_witness_script() {
                    psbt_change_output.witness_script = Some(lock_script.into());
                }
            }

            psbt_change_output.bip32_derivation = bip32_derivation;
            psbt_outputs.push(psbt_change_output);
        }

        Ok(Psbt {
            psbt_version: PsbtVersion::V0,
            tx_version: 2,
            xpub,
            inputs: psbt_inputs,
            outputs: psbt_outputs,
            fallback_locktime: None,
            proprietary: none!(),
            unknown: none!(),
        })
    }
}
