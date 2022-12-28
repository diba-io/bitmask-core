use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Error, Result};
use bdk::{
    blockchain::Blockchain, database::AnyDatabase, miniscript::Descriptor, SignOptions, Wallet,
};
use bitcoin::{
    consensus::serialize,
    hashes::{hex::ToHex, Hash},
    util::{psbt::PartiallySignedTransaction, taproot::TapBranchHash},
    Transaction,
};

use commit_verify::{lnpbp4::CommitmentHash, CommitVerify, TaggedHash};
use electrum_client::{Client, ElectrumApi};
use psbt::Psbt;
use regex::Regex;
use std::str::FromStr;
use wallet::{
    descriptors::InputDescriptor,
    scripts::{
        taproot::{Node, TreeNode},
        PubkeyScript, TapScript,
    },
};

use crate::{
    data::{constants::BITCOIN_ELECTRUM_API, structs::AddressAmount},
    debug,
    operations::bitcoin::balance::get_blockchain,
    FullUtxo,
};

pub async fn create_psbt(
    btc_descriptor: String,
    inputs: Vec<FullUtxo>,
    outputs: Vec<AddressAmount>,
    fee: u64,
) -> Result<PartiallySignedTransaction> {
    let mut input_desc = vec![];
    for input in inputs.clone() {
        // TODO: This is a workaround.
        // Remove after new descriptor system is done.
        // Apply TapRet Commitment
        let tweak = if !input.commitment.trim().is_empty() {
            let commit = CommitmentHash::from_hex(&input.commitment.to_string()).expect("");
            let script_commitment = TapScript::commit(&(commit, 0));
            let root = TreeNode::with_tap_script(script_commitment, 0);
            let tweak = TapBranchHash::from_inner(root.node_hash().into_inner());
            format!("tapret:{}", tweak.to_hex())
        } else {
            "".to_owned()
        };

        let descriptor = format!(
            "{}:{} {} {}",
            input.utxo.outpoint.txid, input.utxo.outpoint.vout, input.terminal_derivation, tweak
        );
        debug!(format!(
            "Parsing InputDescriptor from outpoint: {descriptor}"
        ));

        let input_descriptor = match InputDescriptor::from_str(&descriptor) {
            Ok(desc) => desc,
            Err(err) => return Err(Error::msg(format!("Error parsing input_descriptor: {err}"))),
        };
        debug!(format!(
            "InputDescriptor successfully parsed: {input_descriptor:#?}"
        ));
        input_desc.push(input_descriptor);
    }

    let txid_set: BTreeSet<_> = inputs
        .into_iter()
        .map(|input| input.utxo.outpoint.txid)
        .collect();
    debug!(format!("txid set: {txid_set:?}"));

    let url = BITCOIN_ELECTRUM_API.read().await;
    let electrum_client = Client::new(&url)?;
    debug!(format!("Electrum client connected to {url}"));

    let tx_map = electrum_client
        .batch_transaction_get(&txid_set)?
        .into_iter()
        .map(|tx| (tx.txid(), tx))
        .collect::<BTreeMap<_, _>>();

    debug!("Create PSBT...");
    // format BDK descriptor for LNPBP descriptor
    let re = Regex::new(r"\(\[([0-9a-f]+)/(.+)](.+?)/")?;
    let cap = re.captures(&btc_descriptor).unwrap();
    let btc_descriptor = format!("tr(m=[{}]/{}=[{}]/*/*)", &cap[1], &cap[2], &cap[3]);
    let btc_descriptor = btc_descriptor.replace('\'', "h");

    debug!(format!(
        "Creating descriptor wallet from BTC Descriptor: {btc_descriptor}"
    ));
    let descriptor = match Descriptor::from_str(&btc_descriptor) {
        Ok(d) => d,
        Err(err) => {
            return Err(Error::msg(format!("Error parsing input_descriptor: {err}")));
        }
    };

    let outputs_desc = outputs
        .iter()
        .map(|a| (PubkeyScript::from(a.address.script_pubkey()), a.amount))
        .collect::<Vec<_>>();

    let psbt = match Psbt::construct(
        &descriptor,
        &input_desc,
        &outputs_desc,
        0_u16,
        fee,
        None,
        &tx_map,
    ) {
        Ok(p) => p,
        Err(err) => {
            return Err(Error::msg(format!(
                "Error constructing PSBT from RGB Tokens Descriptor: {err}",
            )));
        }
    };

    Ok(psbt.into())
}

pub async fn sign_psbt(
    wallet: &Wallet<AnyDatabase>,
    mut psbt: PartiallySignedTransaction,
) -> Result<Transaction> {
    debug!("Signing PSBT...");
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    debug!(format!("Finalized: {finalized}"));
    if finalized {
        debug!("Signed PSBT:", base64::encode(&serialize(&psbt)));
        let tx = psbt.extract_tx();
        debug!("tx:", base64::encode(&serialize(&tx)));
        let blockchain = get_blockchain();
        blockchain.broadcast(&tx).await?;
        Ok(tx)
    } else {
        Err(Error::msg("Could not finalize when signing PSBT"))
    }
}
