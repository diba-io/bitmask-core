use std::str::FromStr;

use amplify::hex::ToHex;
use bdk::wallet::coin_selection::{decide_change, Excess};
use bdk::FeeRate;
use bitcoin::secp256k1::SECP256K1;
use bitcoin::{EcdsaSighashType, OutPoint, Script, XOnlyPublicKey};
use bitcoin_blockchain::locks::SeqNo;
use bitcoin_scripts::PubkeyScript;
use bp::dbc::tapret::TapretCommitment;
use commit_verify::mpc::Commitment;
use miniscript_crate::Descriptor;
use psbt::{ProprietaryKey, ProprietaryKeyType};
use rgb::psbt::{
    DbcPsbtError, TapretKeyError, PSBT_OUT_TAPRET_COMMITMENT, PSBT_OUT_TAPRET_HOST,
    PSBT_TAPRET_PREFIX,
};
use rgb::{RgbDescr, RgbWallet, TerminalPath};
use wallet::descriptors::derive::DeriveDescriptor;
use wallet::hd::DerivationSubpath;
use wallet::psbt::Psbt;
use wallet::{
    descriptors::InputDescriptor,
    hd::{DerivationAccount, UnhardenedIndex},
    onchain::ResolveTx,
    psbt::{ProprietaryKeyDescriptor, ProprietaryKeyError, ProprietaryKeyLocation},
};

use crate::bitcoin::{get_wallet, synchronize_wallet};
use crate::rgb::{constants::RGB_PSBT_TAPRET, structs::AddressAmount};

#[allow(clippy::too_many_arguments)]
pub fn create_psbt(
    descriptor_pub: String,
    asset_utxo: String,
    asset_utxo_terminal: String,
    change_index: Option<u16>,
    bitcoin_changes: Vec<String>,
    fee: u64,
    _tap_tweak: Option<String>,
    tx_resolver: &impl ResolveTx,
) -> Result<(Psbt, String), ProprietaryKeyError> {
    let outpoint: OutPoint = asset_utxo.parse().expect("invalid outpoint parse");
    let input = InputDescriptor {
        outpoint,
        terminal: asset_utxo_terminal
            .parse()
            .expect("invalid terminal path parse"),
        seq_no: SeqNo::default(),
        tweak: None,
        sighash_type: EcdsaSighashType::All,
    };

    let bitcoin_addresses: Vec<AddressAmount> = bitcoin_changes
        .into_iter()
        .map(|btc| AddressAmount::from_str(btc.as_str()).expect("invalid AddressFormat parse"))
        .collect();

    let outputs: Vec<(PubkeyScript, u64)> = bitcoin_addresses
        .into_iter()
        .map(|AddressAmount { address, amount }| (address.script_pubkey().into(), amount))
        .collect();

    // to avoid derivation mismatched
    let contract_index = input.terminal.first().expect("first derivation index");
    let terminal_step = format!("/{contract_index}/*");

    let descriptor_pub = descriptor_pub.replace(&terminal_step, "/*/*");
    let descriptor: &Descriptor<DerivationAccount> =
        &Descriptor::from_str(&descriptor_pub).expect("invalid descriptor parse");

    let proprietary_keys = vec![ProprietaryKeyDescriptor {
        location: ProprietaryKeyLocation::Output(0_u16),
        ty: ProprietaryKeyType {
            prefix: RGB_PSBT_TAPRET.to_owned(),
            subtype: 0,
        },
        key: None,
        value: None,
    }];

    let mut change_derivation = vec![input.terminal[0]];
    let change_index = match change_index {
        Some(index) => {
            UnhardenedIndex::from_str(&index.to_string()).expect("invalid change_index parse")
        }
        _ => UnhardenedIndex::default(),
    };
    change_derivation.insert(1, change_index);

    let change_terminal = format!("/{contract_index}/{change_index}");

    let inputs = vec![input];
    let mut psbt = Psbt::construct(
        descriptor,
        &inputs,
        &outputs,
        change_derivation,
        fee,
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
pub async fn estimate_fee_tx(
    descriptor_pub: &str,
    asset_utxo: &str,
    asset_utxo_terminal: &str,
    change_index: Option<u16>,
    bitcoin_changes: Vec<String>,
) -> u64 {
    let outpoint = OutPoint::from_str(asset_utxo).expect("invalid outpoint");
    let wallet = get_wallet(descriptor_pub, None)
        .await
        .expect("cannot retrieve wallet");

    synchronize_wallet(&wallet)
        .await
        .expect("cannot sync wallet");

    let local = wallet.get_utxo(outpoint);
    let local = local.expect("utxo not found").unwrap();

    let change_index = match change_index {
        Some(index) => {
            UnhardenedIndex::from_str(&index.to_string()).expect("invalid change_index parse")
        }
        _ => UnhardenedIndex::default(),
    };

    // Other Recipient
    let mut total_spent = 0;
    for bitcoin_change in bitcoin_changes {
        let recipient =
            AddressAmount::from_str(&bitcoin_change).expect("invalid address amount format");
        total_spent += recipient.amount;
    }

    // Main Recipient
    total_spent = local.txout.value - total_spent;
    let target_script = get_recipient_script(descriptor_pub, asset_utxo_terminal, change_index)
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
    descriptor_pub: &str,
    asset_utxo_terminal: &str,
    change_index: UnhardenedIndex,
) -> Option<Script> {
    let contract_terminal: DerivationSubpath<UnhardenedIndex> = asset_utxo_terminal
        .parse()
        .expect("invalid terminal path parse");

    let contract_index = contract_terminal.first().expect("first derivation index");
    let terminal_step = format!("/{contract_index}/*");
    let descriptor_pub = descriptor_pub.replace(&terminal_step, "/*/*");
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
