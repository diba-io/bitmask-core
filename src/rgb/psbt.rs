use std::collections::BTreeMap;
use std::str::FromStr;

use amplify::hex::ToHex;
use bdk::wallet::coin_selection::{decide_change, Excess};
use bdk::FeeRate;
use bitcoin::secp256k1::SECP256K1;
use bitcoin::util::bip32::Fingerprint;
use bitcoin::{EcdsaSighashType, OutPoint, Script, XOnlyPublicKey};
// TODO: Incompatible versions between RGB and Descriptor Wallet
use bitcoin_30::secp256k1::SECP256K1 as SECP256K1_30;
use bitcoin_30::taproot::TaprootBuilder;
use bitcoin_30::ScriptBuf;
use bitcoin_blockchain::locks::SeqNo;
use bitcoin_scripts::PubkeyScript;
use bp::dbc::tapret::TapretCommitment;
use bp::TapScript;
use commit_verify::mpc::Commitment;
use commit_verify::CommitVerify;

use miniscript_crate::Descriptor;
use psbt::{ProprietaryKey, ProprietaryKeyType};
use rgb::psbt::{
    DbcPsbtError, TapretKeyError, PSBT_OUT_TAPRET_COMMITMENT, PSBT_OUT_TAPRET_HOST,
    PSBT_TAPRET_PREFIX,
};
use rgb::{Resolver, RgbDescr, RgbWallet, TerminalPath};
use wallet::descriptors::derive::DeriveDescriptor;
use wallet::hd::DerivationSubpath;
use wallet::psbt::Psbt;
use wallet::{
    descriptors::InputDescriptor,
    hd::{DerivationAccount, UnhardenedIndex},
    onchain::ResolveTx,
    psbt::{ProprietaryKeyDescriptor, ProprietaryKeyError, ProprietaryKeyLocation},
};

use crate::rgb::{constants::RGB_PSBT_TAPRET, structs::AddressAmount};
use crate::structs::SecretString;

#[allow(clippy::too_many_arguments)]
pub fn create_psbt(
    descriptor_pub: String,
    asset_utxos: Vec<(String, String, Option<String>)>,
    bitcoin_change_index: Option<u16>,
    bitcoin_changes: Vec<String>,
    bitcoin_fee: u64,
    wallet: Option<RgbWallet>,
    tx_resolver: &impl ResolveTx,
) -> Result<(Psbt, String), ProprietaryKeyError> {
    let mut inputs = vec![];

    // Define Descriptor
    let (_, terminal, _) = asset_utxos.first().unwrap();
    let terminal: DerivationSubpath<UnhardenedIndex> =
        terminal.parse().expect("invalid terminal path parse");

    let contract_index = terminal.first().expect("first derivation index");
    let terminal_step = format!("/{contract_index}/*");

    let descriptor_pub = descriptor_pub.replace(&terminal_step, "/*/*");
    let descriptor: &Descriptor<DerivationAccount> =
        &Descriptor::from_str(&descriptor_pub).expect("invalid descriptor parse");

    // Define Input Descriptors
    for (asset_utxo, asset_utxo_terminal, tapret) in asset_utxos {
        let outpoint: OutPoint = asset_utxo.parse().expect("invalid outpoint parse");
        let mut input = InputDescriptor {
            outpoint,
            terminal: asset_utxo_terminal
                .parse()
                .expect("invalid terminal path parse"),
            seq_no: SeqNo::default(),
            tweak: None,
            sighash_type: EcdsaSighashType::All,
        };

        // Verify TapTweak (User Input or Watcher inspect)
        if let Some(tapret) = tapret {
            input.tweak = Some((
                Fingerprint::default(),
                tapret.parse().expect("invalid hash"),
            ))
        } else if let Some(tweak) = complete_input_desc(
            descriptor.clone(),
            input.clone(),
            wallet.clone(),
            tx_resolver,
        )
        .expect("complete descriptor error")
        {
            input.tweak = Some((Fingerprint::default(), tweak.parse().expect("invalid hash")))
        }

        inputs.push(input);
    }

    let bitcoin_addresses: Vec<AddressAmount> = bitcoin_changes
        .into_iter()
        .map(|btc| AddressAmount::from_str(btc.as_str()).expect("invalid AddressFormat parse"))
        .collect();

    let outputs: Vec<(PubkeyScript, u64)> = bitcoin_addresses
        .into_iter()
        .map(|AddressAmount { address, amount }| (address.script_pubkey().into(), amount))
        .collect();

    // Define Tapret Proprierties
    let proprietary_keys = vec![ProprietaryKeyDescriptor {
        location: ProprietaryKeyLocation::Output(0_u16),
        ty: ProprietaryKeyType {
            prefix: RGB_PSBT_TAPRET.to_owned(),
            subtype: 0,
        },
        key: None,
        value: None,
    }];

    // Change Terminal Derivation
    let mut change_derivation = vec![terminal[0]];
    let change_index = match bitcoin_change_index {
        Some(index) => {
            UnhardenedIndex::from_str(&index.to_string()).expect("invalid change_index parse")
        }
        _ => UnhardenedIndex::default(),
    };
    change_derivation.insert(1, change_index);
    let change_terminal = format!("/{contract_index}/{change_index}");

    let mut psbt = Psbt::construct(
        descriptor,
        &inputs,
        &outputs,
        change_derivation,
        bitcoin_fee,
        tx_resolver,
    )
    .expect("cannot be construct PSBT information");

    for key in proprietary_keys {
        match key.location {
            ProprietaryKeyLocation::Input(pos) if pos as usize >= psbt.inputs.len() => {
                return Err(ProprietaryKeyError::InputOutOfRange(pos, psbt.inputs.len()))
            }
            ProprietaryKeyLocation::Output(pos) if pos as usize >= psbt.outputs.len() => {
                return Err(ProprietaryKeyError::OutputOutOfRange(
                    pos,
                    psbt.inputs.len(),
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

    Ok((psbt, change_terminal))
}

fn complete_input_desc(
    descriptor: Descriptor<DerivationAccount>,
    input: InputDescriptor,
    wallet: Option<RgbWallet>,
    tx_resolver: &impl ResolveTx,
) -> anyhow::Result<Option<String>> {
    let txid = input.outpoint.txid;
    let tx = tx_resolver.resolve_tx(txid).expect("tx not found");
    let prev_output = tx.output.get(input.outpoint.vout as usize).unwrap();

    let mut scripts = bmap![];
    if let Descriptor::Tr(_) = descriptor {
        let output_descriptor = DeriveDescriptor::<XOnlyPublicKey>::derive_descriptor(
            &descriptor,
            SECP256K1,
            &input.terminal,
        )
        .expect("invalid descriptor");

        scripts.insert(output_descriptor.script_pubkey(), None);

        if let Some(walet) = wallet {
            let RgbDescr::Tapret(tapret_desc) = walet.descr;
            let contract_index = u32::from_str(
                &input
                    .terminal
                    .first()
                    .expect("first derivation index")
                    .to_string(),
            )
            .expect("invalid parse");
            let change_index = u32::from_str(
                &input
                    .terminal
                    .last()
                    .expect("first derivation index")
                    .to_string(),
            )
            .expect("invalid parse");

            let terminal = TerminalPath {
                app: contract_index,
                index: change_index,
            };

            if let Some(taprets) = tapret_desc.taprets.get(&terminal) {
                taprets.iter().for_each(|tweak| {
                    if let Descriptor::<XOnlyPublicKey>::Tr(tr_desc) = &output_descriptor {
                        let xonly = tr_desc.internal_key();

                        let tap_tweak = ScriptBuf::from_bytes(TapScript::commit(tweak).to_vec());
                        let tap_builder = TaprootBuilder::with_capacity(1)
                            .add_leaf(0, tap_tweak)
                            .expect("complete tree");

                        // TODO: Incompatible versions between RGB and Descriptor Wallet
                        let xonly_30 =
                            bitcoin_30::secp256k1::XOnlyPublicKey::from_str(&xonly.to_hex())
                                .expect("");

                        let spent_info = tap_builder
                            .finalize(SECP256K1_30, xonly_30)
                            .expect("complete tree");

                        let merkle_root = spent_info.merkle_root().expect("script tree present");
                        let tap_script =
                            ScriptBuf::new_v1_p2tr(SECP256K1_30, xonly_30, Some(merkle_root));

                        let spk = Script::from_str(&tap_script.as_script().to_hex()).expect("msg");
                        scripts.insert(spk, Some(merkle_root.to_hex()));
                    }
                });
            }
        }
    };

    let result = scripts
        .into_iter()
        .find(|(sc, _)| sc.clone() == prev_output.script_pubkey)
        .expect("derived scriptPubkey does not match transaction scriptPubkey");
    Ok(result.1)
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
pub fn estimate_fee_tx<T>(
    descriptor_pub: &SecretString,
    bitcoin_inputs: BTreeMap<String, String>,
    bitcoin_changes: Vec<String>,
    bitcoin_change_index: Option<u16>,
    resolver: &mut T,
) -> u64
where
    T: ResolveTx + Resolver,
{
    let mut vout_value = 0;
    let mut bitcoin_terminal = String::new();

    for (utxo, terminal) in bitcoin_inputs {
        let outpoint = OutPoint::from_str(&utxo).expect("invalid outpoint");
        if let Ok(tx) = resolver.resolve_tx(outpoint.txid) {
            if let Some(vout) = tx.output.to_vec().get(outpoint.vout as usize) {
                vout_value = vout.value;
            }

            if bitcoin_terminal.is_empty() {
                bitcoin_terminal = terminal;
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

    let bitcoin_change_index = match bitcoin_change_index {
        Some(index) => index.into(),
        _ => UnhardenedIndex::default(),
    };

    // Main Recipient
    total_spent = vout_value - total_spent;
    let target_script =
        get_recipient_script(descriptor_pub, &bitcoin_terminal, bitcoin_change_index)
            .expect("invalid derivation");

    // TODO: Provide way to get fee rate estimate
    let fee_rate = FeeRate::from_sat_per_vb(5.0);
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
    descriptor_pub: &SecretString,
    bitcoin_terminal: &str,
    change_index: UnhardenedIndex,
) -> Option<Script> {
    let bitcoin_terminal: DerivationSubpath<UnhardenedIndex> = bitcoin_terminal
        .parse()
        .expect("invalid terminal path parse");

    let contract_index = bitcoin_terminal.first().expect("first derivation index");
    let terminal_step = format!("/{contract_index}/*");

    let descriptor_pub = descriptor_pub.0.replace(&terminal_step, "/*/*");
    let descriptor: &Descriptor<DerivationAccount> =
        &Descriptor::from_str(&descriptor_pub).expect("invalid descriptor parse");

    let change_derivation = [*contract_index, change_index];
    match descriptor {
        Descriptor::Tr(_) => {
            let change_descriptor = DeriveDescriptor::<XOnlyPublicKey>::derive_descriptor(
                descriptor,
                SECP256K1,
                change_derivation,
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
}
