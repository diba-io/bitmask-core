#![allow(unused_imports)]
#![allow(unused_variables)]
use amplify::{
    confinement::Confined,
    hex::{FromHex, ToHex},
};
use bdk::blockchain::EsploraBlockchain;
use bech32::{decode, FromBase32};
use bitcoin::{OutPoint, Script, Txid};
use bitcoin_30::ScriptBuf;
use bp::{LockTime, Outpoint, SeqNo, Tx, TxIn, TxOut, TxVer, Txid as BpTxid, VarIntArray, Witness};
use rgb::{DeriveInfo, MiningStatus, RgbWallet, SpkDescriptor, Utxo};
use rgbstd::containers::Contract;
use std::{collections::BTreeMap, str::FromStr};
use strict_encoding::StrictDeserialize;
use wallet::onchain::ResolveTx;

use super::resolvers::ExplorerResolver;
#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolve_commit_utxo(contract: &str, explorer: &mut ExplorerResolver) {}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolve_psbt_tx(asset_utxo: &str, explorer: &mut ExplorerResolver) {}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolve_spend(
    iface_index: u32,
    wallet: RgbWallet,
    explorer: &mut ExplorerResolver,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolve_watcher(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolve_txs(txids: Vec<Txid>, explorer: &mut ExplorerResolver) {}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolve_commit_utxo(contract: &str, explorer: &mut ExplorerResolver) {
    let esplora_client: EsploraBlockchain = EsploraBlockchain::new(&explorer.explorer_url, 100);
    let serialized = if contract.starts_with("rgb1") {
        let (_, serialized, _) =
            decode(contract).expect("invalid serialized contract (bech32m format)");
        Vec::<u8>::from_base32(&serialized).expect("invalid hexadecimal contract (bech32m format)")
    } else {
        Vec::<u8>::from_hex(contract).expect("invalid hexadecimal contract (baid58 format)")
    };

    let confined = Confined::try_from_iter(serialized.iter().copied())
        .expect("invalid strict serialized data");
    let contract = Contract::from_strict_serialized::<{ usize::MAX }>(confined)
        .expect("invalid strict contract data");

    for anchor_bundle in contract.bundles {
        let transaction_id = &bitcoin::Txid::from_str(&anchor_bundle.anchor.txid.to_hex())
            .expect("invalid transaction ID");

        let tx_raw = esplora_client
            .get_tx(transaction_id)
            .await
            .expect("service unavaliable");

        if let Some(tx) = tx_raw {
            let new_tx = Tx {
                version: TxVer::from_consensus_i32(tx.version),
                inputs: VarIntArray::try_from_iter(tx.input.into_iter().map(|txin| {
                    TxIn {
                        prev_output: Outpoint::new(
                            BpTxid::from_str(&txin.previous_output.txid.to_hex())
                                .expect("invalid transaction ID"),
                            txin.previous_output.vout,
                        ),
                        sig_script: txin.script_sig.to_bytes().into(),
                        sequence: SeqNo::from_consensus_u32(txin.sequence.to_consensus_u32()),
                        witness: Witness::from_consensus_stack(txin.witness.to_vec()),
                    }
                }))
                .expect("consensus-invalid transaction"),
                outputs: VarIntArray::try_from_iter(tx.output.into_iter().map(|txout| TxOut {
                    value: txout.value.into(),
                    script_pubkey: txout.script_pubkey.to_bytes().into(),
                }))
                .expect("consensus-invalid transaction"),
                lock_time: LockTime::from_consensus_u32(tx.lock_time.0),
            };

            explorer.bp_txs.insert(anchor_bundle.anchor.txid, new_tx);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolve_psbt_tx(asset_utxo: &str, explorer: &mut ExplorerResolver) {
    let esplora_client: EsploraBlockchain = EsploraBlockchain::new(&explorer.explorer_url, 100);

    let outpoint: OutPoint = asset_utxo.parse().expect("invalid outpoint format");
    let txid = outpoint.txid;
    if let Some(tx) = esplora_client
        .get_tx(&txid)
        .await
        .expect("service unavaliable")
    {
        explorer.txs.insert(txid, tx);
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolve_spend(
    iface_index: u32,
    wallet: RgbWallet,
    explorer: &mut ExplorerResolver,
) {
    let esplora_client: EsploraBlockchain = EsploraBlockchain::new(&explorer.explorer_url, 100);
    let utxos: Vec<Utxo> = wallet
        .utxos
        .into_iter()
        .filter(|utxo| {
            utxo.derivation.terminal.app == iface_index && utxo.derivation.tweak.is_none()
        })
        .collect();

    if !utxos.is_empty() {
        for utxo in utxos {
            let txid = bitcoin_hashes::hex::FromHex::from_hex(&utxo.outpoint.txid.to_hex())
                .expect("invalid outpoint format");
            if let Some(status) = esplora_client
                .get_output_status(&txid, utxo.outpoint.vout.into_u32().into())
                .await
                .expect("service unavaliable")
            {
                if status.spent {
                    explorer.next_utxo = utxo.outpoint.to_string();
                    break;
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolve_watcher(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
) {
    let esplora_client: EsploraBlockchain = EsploraBlockchain::new(&explorer.explorer_url, 100);

    let step = 20;
    let mut index = 0;

    let iface_indexes: Vec<u32> = wallet
        .utxos
        .clone()
        .into_iter()
        .filter(|utxo| utxo.derivation.terminal.app == iface_index)
        .map(|utxo| utxo.derivation.terminal.index)
        .collect();

    loop {
        let scripts = wallet.descr.derive(iface_index, index..step);
        let new_scripts: BTreeMap<DeriveInfo, ScriptBuf> = scripts
            .into_iter()
            .filter(|(d, _)| !iface_indexes.contains(&d.terminal.index))
            .map(|(d, sc)| (d, sc))
            .collect();

        let mut utxos = bset![];
        let script_list = new_scripts.into_iter().map(|(d, sc)| {
            (
                d,
                Script::from_str(&sc.to_hex_string()).expect("invalid script"),
            )
        });

        for (derive, script) in script_list {
            let txs = match esplora_client.scripthash_txs(&script, none!()).await {
                Ok(txs) => txs,
                _ => vec![],
            };

            txs.into_iter().for_each(|tx| {
                let index = tx
                    .vout
                    .clone()
                    .into_iter()
                    .position(|txout| txout.scriptpubkey == script);
                if let Some(index) = index {
                    let index = index;

                    let status = match tx.status.block_height {
                        Some(height) => MiningStatus::Blockchain(height),
                        _ => MiningStatus::Mempool,
                    };
                    let outpoint = Outpoint::new(
                        bp::Txid::from_str(&tx.txid.to_hex()).expect("invalid transactionID parse"),
                        index as u32,
                    );
                    let new_utxo = Utxo {
                        outpoint,
                        status,
                        amount: tx.vout[index].value,
                        derivation: derive.clone(),
                    };
                    utxos.insert(new_utxo);
                }
            });
        }

        if utxos.is_empty() {
            break;
        }
        wallet.utxos.append(&mut utxos);
        index += step;
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolve_txs(txids: Vec<Txid>, explorer: &mut ExplorerResolver) {
    let esplora_client = EsploraBlockchain::new(&explorer.explorer_url, 100);
    for txid in txids {
        if let Some(tx) = esplora_client
            .get_tx(&txid)
            .await
            .expect("service unavaliable")
        {
            explorer.txs.insert(txid, tx);
        }
    }
}
