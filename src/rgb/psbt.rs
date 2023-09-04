use std::str::FromStr;

use amplify::hex::{FromHex, ToHex};
use bdk::FeeRate;
use bitcoin::{
    hashes::sha256,
    secp256k1::SECP256K1,
    util::bip32::{self, Fingerprint},
};
use bitcoin::{EcdsaSighashType, OutPoint, Script, XOnlyPublicKey};
// TODO: Incompatible versions between RGB and Descriptor Wallet
use bitcoin_30::{secp256k1::SECP256K1 as SECP256K1_30, taproot::TaprootBuilder, ScriptBuf};
use bitcoin_blockchain::locks::SeqNo;
use bitcoin_scripts::PubkeyScript;
use bp::{dbc::tapret::TapretCommitment, Outpoint, TapScript, Vout};
use commit_verify::{mpc::Commitment, CommitVerify};
use miniscript_crate::Descriptor;
use psbt::{ProprietaryKey, ProprietaryKeyType};
use rgb::{
    psbt::{
        DbcPsbtError, TapretKeyError, PSBT_OUT_TAPRET_COMMITMENT, PSBT_OUT_TAPRET_HOST,
        PSBT_TAPRET_PREFIX,
    },
    DeriveInfo, MiningStatus, Resolver, RgbDescr, RgbWallet, TerminalPath, Utxo,
};
use wallet::{
    descriptors::{derive::DeriveDescriptor, InputDescriptor},
    hd::{DerivationAccount, DerivationSubpath, UnhardenedIndex},
    onchain::ResolveTx,
    psbt::{ProprietaryKeyDescriptor, ProprietaryKeyError, ProprietaryKeyLocation, Psbt},
};

use crate::{
    info,
    rgb::{constants::RGB_PSBT_TAPRET, structs::AddressAmount},
    structs::{AssetType, PsbtInputRequest},
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

    let mut psbt = Psbt::construct(
        global_descriptor,
        &inputs,
        &outputs,
        change_index.to_vec(),
        bitcoin_fee,
        tx_resolver,
    )
    .map_err(|op| CreatePsbtError::Incomplete(op.to_string()))?;

    // Define Tapret Proprierties
    let proprietary_keys = vec![ProprietaryKeyDescriptor {
        location: ProprietaryKeyLocation::Output((psbt.outputs.len() - 1) as u16),
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

    Ok((psbt, change_index.to_string()))
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

pub fn save_commit(outpoint: Outpoint, commit: Vec<u8>, terminal: &str, wallet: &mut RgbWallet) {
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

    let change = psbt_inputs_total - total_psbt_output - amount_change.unwrap_or_default();

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
    let mut fee = fee_rate.fee_wu(pre_psbt.to_unsigned_tx().weight());
    fee += fee_rate.fee_wu(2);
    fee += fee_rate.fee_wu(
        (TXIN_BASE_WEIGHT
            + global_descriptor
                .max_satisfaction_weight()
                .unwrap_or_default())
            * inputs.len(),
    );

    info!(format!(
        "Remaining/Change (Fee): {change}/{} ({fee})",
        amount_change.unwrap_or_default()
    ));

    Ok((change - fee, fee))
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
        let outpoint: OutPoint = psbt_input.utxo.parse().expect("invalid outpoint parse");
        let mut input: InputDescriptor = InputDescriptor {
            outpoint,
            terminal: psbt_input
                .utxo_terminal
                .parse::<DerivationSubpath<UnhardenedIndex>>()
                .map_err(|_| PsbtInputError::WrongTerminal)?,
            seq_no: SeqNo::default(),
            tweak: None,
            sighash_type: EcdsaSighashType::All,
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

                        if let Ok(tap_builder) = TaprootBuilder::with_capacity(1)
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
