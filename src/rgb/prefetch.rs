#![allow(unused_imports)]
#![allow(unused_variables)]
use crate::debug;
use crate::rgb::resolvers::ExplorerResolver;
use crate::structs::AssetType;
use amplify::{
    confinement::Confined,
    hex::{FromHex, ToHex},
};
use bdk::blockchain::EsploraBlockchain;
use bech32::{decode, FromBase32};
use bitcoin::{OutPoint, Script, Txid};
use bitcoin_30::ScriptBuf;
use bitcoin_scripts::{
    address::{AddressCompat, AddressNetwork},
    PubkeyScript,
};
use bp::{LockTime, Outpoint, SeqNo, Tx, TxIn, TxOut, TxVer, Txid as BpTxid, VarIntArray, Witness};
use rgb::{DeriveInfo, MiningStatus, RgbWallet, SpkDescriptor, Utxo};
use rgbstd::containers::Contract;
use std::{collections::BTreeMap, str::FromStr};
use strict_encoding::StrictDeserialize;
use wallet::onchain::ResolveTx;

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_rgb(
    contract: &str,
    explorer: &mut ExplorerResolver,
    asset_type: Option<AssetType>,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_import_rgb(
    contract: &str,
    asset_type: AssetType,
    explorer: &mut ExplorerResolver,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_psbt(input_utxo: &str, explorer: &mut ExplorerResolver) {}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_utxo_status(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_utxos(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_txs(txids: Vec<Txid>, explorer: &mut ExplorerResolver) {}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_waddress(
    address: &str,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    limit: Option<u32>,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_wutxo(
    utxo: &str,
    network: AddressNetwork,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    limit: Option<u32>,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_tx_height(txid: bp::Txid, explorer: &mut ExplorerResolver) {}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_rgb(
    contract: &str,
    explorer: &mut ExplorerResolver,
    asset_type: Option<AssetType>,
) {
    use crate::rgb::import::contract_from_genesis;
    use amplify::confinement::U32;
    use rgbstd::contract::Genesis;

    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);
    let serialized = if contract.starts_with("rgb1") {
        let (_, serialized, _) =
            decode(contract).expect("invalid serialized contract (bech32m format)");
        Vec::<u8>::from_base32(&serialized).expect("invalid hexadecimal contract (bech32m format)")
    } else {
        Vec::<u8>::from_hex(contract).expect("invalid hexadecimal contract (baid58 format)")
    };

    let confined: Confined<Vec<u8>, 0, { U32 }> =
        Confined::try_from_iter(serialized.iter().copied())
            .expect("invalid strict serialized data");

    let contract = match asset_type {
        Some(asset_type) => match Genesis::from_strict_serialized::<{ U32 }>(confined.clone()) {
            Ok(genesis) => contract_from_genesis(genesis, asset_type, None),
            Err(_) => Contract::from_strict_serialized::<{ U32 }>(confined)
                .expect("invalid strict contract data"),
        },
        _ => Contract::from_strict_serialized::<{ U32 }>(confined)
            .expect("invalid strict contract data"),
    };

    for anchor_bundle in contract.bundles {
        let transaction_id = &bitcoin::Txid::from_str(&anchor_bundle.anchor.txid.to_hex())
            .expect("invalid transaction ID");

        let tx_raw = esplora_client
            .get_tx(transaction_id)
            .await
            .expect("service unavaliable");

        if let Some(tx) = tx_raw {
            let txid =
                rgbstd::Txid::from_hex(&transaction_id.to_hex()).expect("invalid transaction id");
            prefetch_resolver_tx_height(txid, explorer).await;
            let new_tx = Tx {
                version: TxVer::from_consensus_i32(tx.clone().version),
                inputs: VarIntArray::try_from_iter(tx.clone().input.into_iter().map(|txin| {
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
                outputs: VarIntArray::try_from_iter(tx.clone().output.into_iter().map(|txout| {
                    TxOut {
                        value: txout.value.into(),
                        script_pubkey: txout.script_pubkey.to_bytes().into(),
                    }
                }))
                .expect("consensus-invalid transaction"),
                lock_time: LockTime::from_consensus_u32(tx.lock_time.0),
            };

            explorer.txs.insert(tx.txid(), tx);
            explorer.bp_txs.insert(anchor_bundle.anchor.txid, new_tx);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_import_rgb(
    contract: &str,
    asset_type: AssetType,
    explorer: &mut ExplorerResolver,
) {
    use crate::rgb::import::contract_from_genesis;
    use amplify::confinement::U32;
    use rgbstd::contract::Genesis;

    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);
    let serialized = if contract.starts_with("rgb1") {
        let (_, serialized, _) =
            decode(contract).expect("invalid serialized contract (bech32m format)");
        Vec::<u8>::from_base32(&serialized).expect("invalid hexadecimal contract (bech32m format)")
    } else {
        Vec::<u8>::from_hex(contract).expect("invalid hexadecimal contract (baid58 format)")
    };

    let confined: Confined<Vec<u8>, 0, { U32 }> =
        Confined::try_from_iter(serialized.iter().copied())
            .expect("invalid strict serialized data");

    let contract = match Genesis::from_strict_serialized::<{ U32 }>(confined.clone()) {
        Ok(genesis) => contract_from_genesis(genesis, asset_type, None),
        Err(_) => Contract::from_strict_serialized::<{ U32 }>(confined)
            .expect("invalid strict contract data"),
    };

    let contract = contract.validate(explorer).expect("invalid contract state");

    for anchor_bundle in contract.bundles {
        let transaction_id = &bitcoin::Txid::from_str(&anchor_bundle.anchor.txid.to_hex())
            .expect("invalid transaction ID");

        let tx_raw = esplora_client
            .get_tx(transaction_id)
            .await
            .expect("service unavaliable");

        if let Some(tx) = tx_raw {
            let new_tx = Tx {
                version: TxVer::from_consensus_i32(tx.clone().version),
                inputs: VarIntArray::try_from_iter(tx.clone().input.into_iter().map(|txin| {
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
                outputs: VarIntArray::try_from_iter(tx.clone().output.into_iter().map(|txout| {
                    TxOut {
                        value: txout.value.into(),
                        script_pubkey: txout.script_pubkey.to_bytes().into(),
                    }
                }))
                .expect("consensus-invalid transaction"),
                lock_time: LockTime::from_consensus_u32(tx.lock_time.0),
            };

            explorer.txs.insert(tx.txid(), tx);
            explorer.bp_txs.insert(anchor_bundle.anchor.txid, new_tx);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_psbt(input_utxo: &str, explorer: &mut ExplorerResolver) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);

    let outpoint: OutPoint = input_utxo.parse().expect("invalid outpoint format");
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
pub async fn prefetch_resolver_utxo_status(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);
    let utxos: Vec<Utxo> = wallet
        .utxos
        .clone()
        .into_iter()
        .filter(|utxo| {
            utxo.derivation.terminal.app == iface_index && utxo.derivation.tweak.is_none()
        })
        .collect();

    if !utxos.is_empty() {
        for utxo in utxos {
            let txid = bitcoin::Txid::from_str(&utxo.outpoint.txid.to_hex())
                .expect("invalid outpoint format");
            if let Some(status) = esplora_client
                .get_output_status(&txid, utxo.outpoint.vout.into_u32().into())
                .await
                .expect("service unavaliable")
            {
                if !status.spent {
                    break;
                }
                explorer.utxos_spent.push(utxo.outpoint.to_string());
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_utxos(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
) {
    use std::collections::HashSet;

    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);

    let step = 100;
    let index = 0;
    let mut utxos = bset![];

    let scripts = wallet.descr.derive(iface_index, index..step);
    let new_scripts: BTreeMap<DeriveInfo, ScriptBuf> =
        scripts.into_iter().map(|(d, sc)| (d, sc)).collect();

    let script_list = new_scripts
        .into_iter()
        .map(|(d, sc)| {
            (
                d,
                Script::from_str(&sc.to_hex_string()).expect("invalid script"),
            )
        })
        .collect::<HashSet<_>>()
        .into_iter();

    for (derive, script) in script_list {
        let mut related_txs = esplora_client
            .scripthash_txs(&script, None)
            .await
            .expect("Service unavaliable");
        let n_confirmed = related_txs.iter().filter(|tx| tx.status.confirmed).count();
        // esplora pages on 25 confirmed transactions. If there are 25 or more we
        // keep requesting to see if there's more.
        if n_confirmed >= 25 {
            loop {
                let new_related_txs = esplora_client
                    .scripthash_txs(&script, Some(related_txs.last().unwrap().txid))
                    .await
                    .expect("Service unavaliable");
                let n = new_related_txs.len();
                related_txs.extend(new_related_txs);
                // we've reached the end
                if n < 25 {
                    break;
                }
            }
        }

        related_txs.into_iter().for_each(|tx| {
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
                    rgbstd::Txid::from_str(&tx.txid.to_hex()).expect("invalid transactionID parse"),
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

    if !utxos.is_empty() {
        wallet.utxos.append(&mut utxos);
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_txs(txids: Vec<Txid>, explorer: &mut ExplorerResolver) {
    let esplora_client = EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);
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

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_waddress(
    address: &str,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    limit: Option<u32>,
) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);

    let mut step = 100;
    if let Some(limit) = limit {
        step = limit;
    }
    let index = 0;

    let sc = AddressCompat::from_str(address).expect("invalid address");
    let script = ScriptBuf::from_hex(&sc.script_pubkey().to_hex()).expect("invalid script");

    let mut scripts: BTreeMap<DeriveInfo, ScriptBuf> = BTreeMap::new();
    let asset_indexes: Vec<u32> = [0, 1, 9, 20, 21].to_vec();
    for app in asset_indexes {
        scripts.append(&mut wallet.descr.derive(app, index..step));
    }

    let script = scripts.into_iter().find(|(_, sc)| sc.eq(&script));
    if let Some((d, sc)) = script {
        let mut scripts = BTreeMap::new();
        scripts.insert(d, sc);

        let script_list = scripts.into_iter().map(|(d, sc)| {
            (
                d,
                Script::from_str(&sc.to_hex_string()).expect("invalid script"),
            )
        });

        let mut utxos = bset![];
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

        if !utxos.is_empty() {
            wallet.utxos.append(&mut utxos);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_wutxo(
    utxo: &str,
    network: AddressNetwork,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    limit: Option<u32>,
) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);

    let outpoint = OutPoint::from_str(utxo).expect("invalid outpoint");

    if let Some(tx) = esplora_client
        .get_tx(&outpoint.txid)
        .await
        .expect("service unavaliable")
    {
        if let Some(vout) = tx.output.to_vec().get(outpoint.vout as usize) {
            let sc = Script::from_str(&vout.script_pubkey.to_hex()).expect("invalid script");
            let pub_script = PubkeyScript::from(sc);
            if let Some(address) = AddressCompat::from_script(&pub_script, network) {
                prefetch_resolver_waddress(&address.to_string(), wallet, explorer, limit).await;
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_tx_height(txid: rgbstd::Txid, explorer: &mut ExplorerResolver) {
    use rgbstd::contract::{WitnessHeight, WitnessOrd};

    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 100).with_concurrency(6);

    let transaction_id =
        &bitcoin::Txid::from_str(&txid.to_hex()).expect("invalid transaction id parse");

    let tx = esplora_client
        .get_tx_status(transaction_id)
        .await
        .expect("service unavaliable");

    if let Some(tx) = tx {
        let status = match tx.block_height {
            Some(height) => WitnessOrd::OnChain(WitnessHeight::new(height).unwrap()),
            _ => WitnessOrd::OffChain,
        };
        explorer.tx_height.insert(txid, status);
    } else {
        explorer.tx_height.insert(txid, WitnessOrd::OffChain);
    }
}
