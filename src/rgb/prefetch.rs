#![allow(unused_imports)]
#![allow(unused_variables)]
use crate::rgb::resolvers::ExplorerResolver;
use crate::structs::{AssetType, TxStatus};
use crate::{debug, structs::IssueMetaRequest, structs::UtxoSpentStatus};
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
use rgbstd::interface::ContractIface;
use std::collections::HashMap;
use std::f32::consts::E;
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
pub async fn prefetch_resolver_user_utxo_status(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    with_block_height: bool,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_allocations(
    contract_iface: ContractIface,
    explorer: &mut ExplorerResolver,
) {
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_resolver_utxos(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    limit: Option<u32>,
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
    use crate::rgb::import::{contract_from_armored, contract_from_other_formats};
    use crate::rgb::prebuild::prebuild_extract_transfer;
    use amplify::confinement::U32;
    use rgbstd::contract::Genesis;

    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);
    let contract = prebuild_extract_transfer(contract).expect("invalid transfer");
    let contract = contract.transfer.unbindle();

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
    use crate::rgb::import::{contract_from_armored, contract_from_other_formats};
    use amplify::confinement::U32;
    use rgbstd::contract::Genesis;

    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);
    let contract = if contract.starts_with("-----BEGIN RGB CONTRACT-----") {
        contract_from_armored(contract)
    } else {
        contract_from_other_formats(contract, Some(asset_type), None)
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
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);

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
pub async fn prefetch_resolver_user_utxo_status(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    with_block_height: bool,
) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);
    let utxos: Vec<Utxo> = wallet
        .utxos
        .clone()
        .into_iter()
        .filter(|utxo| utxo.derivation.terminal.app == iface_index)
        .collect();

    if !utxos.is_empty() {
        for utxo in utxos {
            let txid = bitcoin::Txid::from_str(&utxo.outpoint.txid.to_hex())
                .expect("invalid outpoint format");

            let mut block_h = TxStatus::NotFound;
            if with_block_height {
                if let Ok(Some(tx_status)) = esplora_client.get_tx_status(&txid).await {
                    if tx_status.confirmed {
                        block_h = TxStatus::Block(tx_status.block_height.unwrap_or_default());
                    } else {
                        block_h = TxStatus::Mempool;
                    }
                }
            }

            let index = utxo.outpoint.vout.into_u32();
            if let Some(output_status) = esplora_client
                .get_output_status(&txid, index.into())
                .await
                .expect("service unavaliable")
            {
                let mut height = TxStatus::NotFound;
                if let Some(status) = output_status.status {
                    if status.confirmed {
                        height = TxStatus::Block(status.block_height.unwrap_or_default());
                    } else {
                        height = TxStatus::Mempool;
                    }
                }

                let utxo_status = UtxoSpentStatus {
                    utxo: format!("{txid}:{index}"),
                    is_spent: output_status.spent,
                    spent_height: height,
                    block_height: block_h,
                };

                explorer.utxos_spent.push(utxo_status);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_utxos(
    iface_index: u32,
    wallet: &mut RgbWallet,
    explorer: &mut ExplorerResolver,
    limit: Option<u32>,
) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);

    let index = 0;
    let mut step = 100;
    if let Some(limit) = limit {
        step = limit;
    }

    let scripts = wallet.descr.derive(iface_index, index..step);
    let mut new_utxos = bset![];
    for (derive, script) in scripts {
        // TODO: Remove that after bitcoin v.30 full compatibility
        let script_compatible =
            Script::from_str(&script.as_script().to_hex_string()).expect("invalid script");

        let mut related_txs = esplora_client
            .scripthash_txs(&script_compatible, None)
            .await
            .expect("Service unavaliable");
        let n_confirmed = related_txs.iter().filter(|tx| tx.status.confirmed).count();
        // esplora pages on 25 confirmed transactions. If there are 25 or more we
        // keep requesting to see if there's more.
        if n_confirmed >= 25 {
            loop {
                let new_related_txs = esplora_client
                    .scripthash_txs(&script_compatible, Some(related_txs.last().unwrap().txid))
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
            for (index, vout) in tx.vout.iter().enumerate() {
                if vout.scriptpubkey != script_compatible {
                    continue;
                }

                let status = match tx.status.block_height {
                    Some(height) => MiningStatus::Blockchain(height),
                    _ => MiningStatus::Mempool,
                };
                let outpoint = Outpoint::new(
                    bp::Txid::from_str(&tx.txid.to_hex()).expect("invalid outpoint parse"),
                    index as u32,
                );
                let new_utxo = Utxo {
                    outpoint,
                    status,
                    amount: vout.value,
                    derivation: derive.clone(),
                };
                new_utxos.insert(new_utxo);
            }
        });
    }

    for mut new_utxo in new_utxos {
        if let Some(current_utxo) = wallet
            .utxos
            .clone()
            .into_iter()
            .find(|u| u.outpoint == new_utxo.outpoint)
        {
            if current_utxo.status == MiningStatus::Mempool {
                wallet.utxos.remove(&current_utxo.clone());
                explorer.utxos.insert(current_utxo.clone());

                new_utxo.derivation = current_utxo.derivation;
                wallet.utxos.insert(new_utxo.clone());
                explorer.utxos.insert(new_utxo);
            }
        } else {
            wallet.utxos.insert(new_utxo.clone());
            explorer.utxos.insert(new_utxo);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_txs(txids: Vec<Txid>, explorer: &mut ExplorerResolver) {
    let esplora_client = EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);
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
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);

    let index = 0;
    let mut step = 100;
    if let Some(limit) = limit {
        step = limit;
    }

    let sc = AddressCompat::from_str(address).expect("invalid address");
    let script = ScriptBuf::from_hex(&sc.script_pubkey().to_hex()).expect("invalid script");

    let mut scripts: BTreeMap<DeriveInfo, ScriptBuf> = BTreeMap::new();
    let asset_indexes: Vec<u32> = [0, 1, 9, 10, 20, 21].to_vec();
    for app in asset_indexes {
        scripts.append(&mut wallet.descr.derive(app, index..step));
    }

    let script = scripts.into_iter().find(|(_, sc)| sc.eq(&script));
    if let Some((d, sc)) = script {
        let mut scripts = BTreeMap::new();
        scripts.insert(d, sc);

        let mut new_utxos = bset![];
        for (derive, script) in scripts {
            // TODO: Remove that after bitcoin v.30 full compatibility
            let script_compatible =
                Script::from_str(&script.to_hex_string()).expect("invalid script");
            let txs = match esplora_client
                .scripthash_txs(&script_compatible, none!())
                .await
            {
                Ok(txs) => txs,
                _ => vec![],
            };

            txs.into_iter().for_each(|tx| {
                let index = tx
                    .vout
                    .clone()
                    .into_iter()
                    .position(|txout| txout.scriptpubkey == script_compatible);
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
                    new_utxos.insert(new_utxo);
                }
            });
        }

        for mut new_utxo in new_utxos {
            if let Some(current_utxo) = wallet
                .utxos
                .clone()
                .into_iter()
                .find(|u| u.outpoint == new_utxo.outpoint)
            {
                if current_utxo.status == MiningStatus::Mempool {
                    wallet.utxos.remove(&current_utxo);

                    new_utxo.derivation = current_utxo.derivation;
                    wallet.utxos.insert(new_utxo);
                }
            } else {
                wallet.utxos.insert(new_utxo);
            }
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
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);

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
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);

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

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_resolver_allocations(
    contract_iface: ContractIface,
    explorer: &mut ExplorerResolver,
) {
    let esplora_client: EsploraBlockchain =
        EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);

    let mut contract_utxos = vec![];

    for owned in &contract_iface.iface.assignments {
        if let Ok(allocations) = contract_iface.fungible(owned.name.clone(), &None) {
            for allocation in allocations {
                contract_utxos.push(allocation.owner);
            }
        }

        if let Ok(allocations) = contract_iface.data(owned.name.clone()) {
            for allocation in allocations {
                contract_utxos.push(allocation.owner);
            }
        }
    }

    if !contract_utxos.is_empty() {
        for utxo in contract_utxos {
            let txid =
                bitcoin::Txid::from_str(&utxo.txid.to_hex()).expect("invalid outpoint format");

            let index = utxo.vout.into_u32();
            if let Some(output_status) = esplora_client
                .get_output_status(&txid, index.into())
                .await
                .expect("service unavaliable")
            {
                let mut height = TxStatus::NotFound;
                if let Some(status) = output_status.status {
                    if status.confirmed {
                        height = TxStatus::Block(status.block_height.unwrap_or_default());
                    } else {
                        height = TxStatus::Mempool;
                    }
                }

                let utxo_status = UtxoSpentStatus {
                    utxo: format!("{txid}:{index}"),
                    is_spent: output_status.spent,
                    spent_height: height,
                    block_height: TxStatus::NotFound,
                };

                explorer.utxos_spent.push(utxo_status);
            }
        }
    }
}

pub async fn prefetch_resolver_images(meta: Option<IssueMetaRequest>) -> BTreeMap<String, Vec<u8>> {
    let mut data = BTreeMap::new();
    if let Some(IssueMetaRequest(meta)) = meta {
        match meta {
            crate::structs::IssueMetadata::UDA(items) => {
                let mut hasher = blake3::Hasher::new();
                let source = items[0].source.clone();
                if let Some(bytes) = retrieve_data(&source).await {
                    hasher.update(&bytes);
                } else {
                    hasher.update(source.as_bytes());
                }
                let uda_data = hasher.finalize();
                data.insert(source, uda_data.as_bytes().to_vec());
            }
            crate::structs::IssueMetadata::Collectible(items) => {
                for item in items {
                    let mut hasher = blake3::Hasher::new();
                    let source = item.media[0].source.clone();
                    if let Some(bytes) = retrieve_data(&source).await {
                        hasher.update(&bytes);
                    } else {
                        hasher.update(source.as_bytes());
                    }
                    let uda_data = hasher.finalize();
                    data.insert(source, uda_data.as_bytes().to_vec());
                }
            }
        }
    }

    data
}

async fn retrieve_data(url: &str) -> Option<Vec<u8>> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await;

    if let Ok(response) = response {
        let status_code = response.status().as_u16();
        if status_code == 200 {
            if let Ok(bytes) = response.bytes().await {
                return Some(bytes.to_vec());
            }
        }
    }

    None
}

pub async fn prefetch_resolver_txs_status(txids: Vec<Txid>, explorer: &mut ExplorerResolver) {
    let esplora_client = EsploraBlockchain::new(&explorer.explorer_url, 1).with_concurrency(6);
    for txid in txids {
        let tx_resp = esplora_client.get_tx_status(&txid).await;
        if tx_resp.is_ok() {
            let mut status = TxStatus::NotFound;
            let tx_resp = tx_resp.unwrap_or_default();
            if let Some(tx_status) = tx_resp {
                if tx_status.confirmed {
                    status = TxStatus::Block(tx_status.block_height.unwrap_or_default());
                } else {
                    status = TxStatus::Mempool;
                }
            }
            explorer.txs_status.insert(txid, status);
        } else {
            let err = match tx_resp.err() {
                Some(err) => err.to_string(),
                None => "unknown explorer error".to_string(),
            };

            let err = TxStatus::Error(err);
            explorer.txs_status.insert(txid, err);
        }
    }
}
