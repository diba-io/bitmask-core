use std::str::FromStr;

use amplify::hex::ToHex;
use bdk::{
    wallet::coin_selection::{decide_change, Excess},
    FeeRate,
};
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
use bp::{dbc::tapret::TapretCommitment, TapScript};
use commit_verify::{mpc::Commitment, CommitVerify};
use miniscript_crate::Descriptor;
use psbt::{ProprietaryKey, ProprietaryKeyType};
use rgb::{
    psbt::{
        DbcPsbtError, TapretKeyError, PSBT_OUT_TAPRET_COMMITMENT, PSBT_OUT_TAPRET_HOST,
        PSBT_TAPRET_PREFIX,
    },
    Resolver, RgbDescr, RgbWallet, TerminalPath,
};
use wallet::{
    descriptors::{derive::DeriveDescriptor, InputDescriptor},
    hd::{DerivationAccount, DerivationSubpath, UnhardenedIndex},
    onchain::ResolveTx,
    psbt::{ProprietaryKeyDescriptor, ProprietaryKeyError, ProprietaryKeyLocation, Psbt},
};

use crate::{
    rgb::{constants::RGB_PSBT_TAPRET, structs::AddressAmount},
    structs::{AssetType, PsbtInputRequest, SecretString},
};

use crate::rgb::structs::AddressFormatParseError;

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
    /// The tweak is invalid: {0}
    WrongTweak(String),
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
        let outpoint: OutPoint = psbt_input.utxo.parse().expect("invalid outpoint parse");
        let mut input = InputDescriptor {
            outpoint,
            terminal: psbt_input
                .utxo_terminal
                .parse::<DerivationSubpath<UnhardenedIndex>>()
                .map_err(CreatePsbtError::WrongTerminal)?,
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
                    .map_err(|op| CreatePsbtError::WrongTapTweak(op.to_string()))?,
            ))
        } else if let Some(tweak) = complete_input_desc(
            global_descriptor.clone(),
            input.clone(),
            wallet.clone(),
            tx_resolver,
        )
        .map_err(|op| CreatePsbtError::WrongTapTweak(op.to_string()))?
        {
            input.tweak = Some((
                Fingerprint::default(),
                tweak
                    .parse::<sha256::Hash>()
                    .map_err(|op| CreatePsbtError::WrongTapTweak(op.to_string()))?,
            ))
        }

        inputs.push(input);
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
                            .map_err(|op| PsbtInputError::WrongTweak(op.to_string()))
                        {
                            // TODO: Incompatible versions between RGB and Descriptor Wallet
                            let xonly_30 =
                                bitcoin_30::secp256k1::XOnlyPublicKey::from_str(&xonly.to_hex())
                                    .map_err(|op| PsbtInputError::WrongTweak(op.to_string()))?;

                            let spent_info =
                                tap_builder.finalize(SECP256K1_30, xonly_30).map_err(|_| {
                                    PsbtInputError::WrongTweak("incomplete tree".to_string())
                                })?;

                            if let Some(merkle_root) = spent_info.merkle_root() {
                                let tap_script = ScriptBuf::new_v1_p2tr(
                                    SECP256K1_30,
                                    xonly_30,
                                    Some(merkle_root),
                                );

                                let spk = Script::from_str(&tap_script.as_script().to_hex())
                                    .map_err(|op| PsbtInputError::WrongTweak(op.to_string()))?;
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

pub fn extract_commit(mut psbt: Psbt) -> Result<Vec<u8>, DbcPsbtError> {
    let (_, output) = psbt
        .outputs
        .iter_mut()
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
        Some(commit) => Ok(commit.to_owned()),
        _ => Err(DbcPsbtError::TapretKey(TapretKeyError::InvalidProof)),
    }
}

pub fn save_commit(terminal: &str, commit: Vec<u8>, wallet: &mut RgbWallet) {
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
    tapret.taprets.insert(terminal, bset! {tap_commit});
    wallet.descr = RgbDescr::Tapret(tapret);
}

// TODO: [Experimental] Review with Diba Team
pub fn fee_estimate<T>(
    assets_inputs: Vec<PsbtInputRequest>,
    asset_descriptor_change: Option<SecretString>,
    asset_terminal_change: Option<String>,
    bitcoin_inputs: Vec<PsbtInputRequest>,
    bitcoin_changes: Vec<String>,
    fee_rate: f32,
    resolver: &mut T,
) -> u64
where
    T: ResolveTx + Resolver,
{
    let mut vout_value = 0;
    let fee_rate = FeeRate::from_sat_per_vb(fee_rate);

    // Total Stats
    let mut all_inputs = assets_inputs;
    all_inputs.extend(bitcoin_inputs);

    for item in all_inputs {
        let outpoint = OutPoint::from_str(&item.utxo).expect("invalid outpoint");
        if let Ok(tx) = resolver.resolve_tx(outpoint.txid) {
            if let Some(vout) = tx.output.to_vec().get(outpoint.vout as usize) {
                vout_value = vout.value;
            }
        }
    }

    // Other Recipient
    let mut total_spent = 0;
    for bitcoin_change in bitcoin_changes {
        let recipient =
            AddressAmount::from_str(&bitcoin_change).expect("invalid address amount format");
        total_spent += recipient.amount;
    }

    // Main Recipient
    total_spent = vout_value - total_spent;
    let target_script = get_recipient_script(asset_descriptor_change, asset_terminal_change)
        .expect("invalid derivation");

    let excess = decide_change(total_spent, fee_rate, &target_script);
    match excess {
        Excess::Change { amount: _, fee } => fee,
        Excess::NoChange {
            dust_threshold: _,
            remaining_amount: _,
            change_fee,
        } => change_fee,
    }
}

fn get_recipient_script(
    descriptor_pub: Option<SecretString>,
    bitcoin_terminal: Option<String>,
) -> Option<Script> {
    let (terminal, terminal_path) = match bitcoin_terminal {
        Some(bitcoin_terminal) => (
            bitcoin_terminal.clone(),
            bitcoin_terminal
                .parse()
                .expect("invalid terminal path parse"),
        ),
        None => ("/0/1".to_string(), DerivationSubpath::new()),
    };

    if let Some(descriptor_pub) = descriptor_pub {
        let descriptor_pub = descriptor_pub.to_string().replace(&terminal, "/*/*");
        let descriptor: &Descriptor<DerivationAccount> =
            &Descriptor::from_str(&descriptor_pub).expect("invalid descriptor parse");

        match descriptor {
            Descriptor::Tr(_) => {
                let change_descriptor = DeriveDescriptor::<XOnlyPublicKey>::derive_descriptor(
                    descriptor,
                    SECP256K1,
                    terminal_path,
                )
                .expect("Derivation mismatch");
                let change_descriptor = match change_descriptor {
                    Descriptor::Tr(tr) => tr,
                    _ => unreachable!(),
                };
                Some(change_descriptor.script_pubkey())
            }
            _ => None,
        }
    } else {
        None
    }
}
