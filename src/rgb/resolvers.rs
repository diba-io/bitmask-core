#![allow(unused_imports)]
#![allow(unused_variables)]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::Infallible,
    str::FromStr,
};

use amplify::hex::ToHex;
use bitcoin::Script;
use bp::{LockTime, Outpoint, SeqNo, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray, Witness};
use rgb::{prelude::DeriveInfo, MiningStatus, TerminalPath, Utxo};
use rgbstd::{
    contract::{WitnessHeight, WitnessOrd},
    resolvers::ResolveHeight,
    validation::ResolveTx as ResolveCommiment,
};
use wallet::onchain::{ResolveTx, TxResolverError};

use crate::structs::TxStatus;

#[derive(Default)]
pub struct ExplorerResolver {
    pub explorer_url: String,
    // Prefetch Data (wasm32)
    pub utxos: BTreeSet<Utxo>,
    pub utxos_spent: Vec<String>,
    pub txs: HashMap<bitcoin::Txid, bitcoin::Transaction>,
    pub bp_txs: HashMap<Txid, Tx>,
    pub tx_height: HashMap<Txid, WitnessOrd>,
    pub txs_status: HashMap<bitcoin::Txid, TxStatus>,
}

impl rgb::Resolver for ExplorerResolver {
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_utxo<'s>(
        &mut self,
        scripts: BTreeMap<DeriveInfo, bitcoin_30::ScriptBuf>,
    ) -> Result<BTreeSet<rgb::prelude::Utxo>, String> {
        use std::collections::HashSet;

        let mut utxos = bset![];
        let explorer_client = esplora_block::Builder::new(&self.explorer_url)
            .build_blocking()
            .expect("service unavaliable");
        // TODO: Remove that after bitcoin v.30 full compatibility

        let script_list = scripts
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
            let mut related_txs = explorer_client
                .scripthash_txs(&script, None)
                .expect("Service unavaliable");
            let n_confirmed = related_txs.iter().filter(|tx| tx.status.confirmed).count();
            // esplora pages on 25 confirmed transactions. If there are 25 or more we
            // keep requesting to see if there's more.
            if n_confirmed >= 25 {
                loop {
                    let new_related_txs = explorer_client
                        .scripthash_txs(&script, Some(related_txs.last().unwrap().txid))
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
                    if vout.scriptpubkey != script {
                        continue;
                    }

                    let status = match tx.status.block_height {
                        Some(height) => MiningStatus::Blockchain(height),
                        _ => MiningStatus::Mempool,
                    };
                    let outpoint = Outpoint::new(
                        Txid::from_str(&tx.txid.to_hex()).expect("invalid outpoint parse"),
                        index as u32,
                    );
                    let new_utxo = Utxo {
                        outpoint,
                        status,
                        amount: vout.value,
                        derivation: derive.clone(),
                    };
                    utxos.insert(new_utxo);
                }
            });
        }
        Ok(utxos)
    }

    #[cfg(target_arch = "wasm32")]
    fn resolve_utxo<'s>(
        &mut self,
        scripts: BTreeMap<DeriveInfo, bitcoin_30::ScriptBuf>,
    ) -> Result<BTreeSet<rgb::prelude::Utxo>, String> {
        Ok(self.utxos.clone())
    }
}

impl ResolveTx for ExplorerResolver {
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_tx(
        &self,
        txid: bitcoin::Txid,
    ) -> Result<bitcoin::Transaction, wallet::onchain::TxResolverError> {
        let explorer_client = esplora_block::Builder::new(&self.explorer_url)
            .build_blocking()
            .expect("service unavaliable");

        match explorer_client.get_tx(&txid).expect("service unavaliable") {
            Some(tx) => Ok(tx),
            _ => Err(TxResolverError { txid, err: none!() }),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn resolve_tx(
        &self,
        txid: bitcoin::Txid,
    ) -> Result<bitcoin::Transaction, wallet::onchain::TxResolverError> {
        match self.txs.get(&txid) {
            Some(tx) => Ok(tx.to_owned()),
            _ => Err(TxResolverError { txid, err: none!() }),
        }
    }
}

impl ResolveHeight for ExplorerResolver {
    type Error = TxResolverError;
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_height(&mut self, txid: Txid) -> Result<WitnessOrd, Self::Error> {
        let esplora_client = esplora_block::Builder::new(&self.explorer_url)
            .build_blocking()
            .expect("service unavaliable");
        let transaction_id =
            &bitcoin::Txid::from_str(&txid.to_hex()).expect("invalid transaction id parse");
        let tx = esplora_client
            .get_tx_status(transaction_id)
            .expect("service unavaliable");

        let status = match tx.block_height {
            Some(height) => WitnessOrd::OnChain(WitnessHeight::new(height).unwrap()),
            _ => WitnessOrd::OffChain,
        };

        Ok(status)
    }

    #[cfg(target_arch = "wasm32")]
    fn resolve_height(&mut self, txid: Txid) -> Result<WitnessOrd, Self::Error> {
        Ok(WitnessOrd::OffChain)
    }
}

// TODO: Review after migrate to rust-bitcoin v0.30
impl ResolveCommiment for ExplorerResolver {
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_tx(&self, txid: Txid) -> Result<Tx, rgbstd::validation::TxResolverError> {
        let explorer_client = esplora_block::Builder::new(&self.explorer_url)
            .build_blocking()
            .expect("service unavaliable");

        let transaction_id =
            &bitcoin::Txid::from_str(&txid.to_hex()).expect("invalid transaction id parse");
        let tx = explorer_client
            .get_tx(transaction_id)
            .expect("service unavaliable");

        match tx {
            Some(tx) => Ok(Tx {
                version: TxVer::from_consensus_i32(tx.version),
                inputs: VarIntArray::try_from_iter(tx.input.into_iter().map(|txin| {
                    TxIn {
                        prev_output: Outpoint::new(
                            Txid::from_str(&txin.previous_output.txid.to_hex())
                                .expect("invalid transaction id parse"),
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
            }),
            _ => Err(rgbstd::validation::TxResolverError::Unknown(txid)),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn resolve_tx(&self, txid: Txid) -> Result<Tx, rgbstd::validation::TxResolverError> {
        match self.bp_txs.get(&txid) {
            Some(tx) => Ok(tx.clone()),
            _ => Err(rgbstd::validation::TxResolverError::Unknown(txid)),
        }
    }
}

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum SpendResolverError {
    /// transaction {0} is not mined
    Unknown(bitcoin::Txid),
}

pub trait ResolveSpent {
    type Error: std::error::Error;

    fn resolve_spent_status(
        &mut self,
        txid: bitcoin::Txid,
        index: u64,
    ) -> Result<bool, Self::Error>;
}

impl ResolveSpent for ExplorerResolver {
    type Error = SpendResolverError;
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_spent_status(
        &mut self,
        txid: bitcoin::Txid,
        index: u64,
    ) -> Result<bool, Self::Error> {
        let explorer_client = esplora_block::Builder::new(&self.explorer_url)
            .build_blocking()
            .expect("service unavaliable");
        match explorer_client
            .get_output_status(&txid, index)
            .expect("service unavaliable")
        {
            Some(status) => Ok(status.spent),
            _ => Err(SpendResolverError::Unknown(txid)),
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn resolve_spent_status(
        &mut self,
        txid: bitcoin::Txid,
        index: u64,
    ) -> Result<bool, Self::Error> {
        let outpoint = format!("{}:{}", txid.to_hex(), index);
        Ok(self.utxos_spent.contains(&outpoint))
    }
}

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ResolverTxStatusError {
    Unknown,
}

pub trait ResolveTxStatus {
    type Error: std::error::Error;

    fn resolve_tx_status(&mut self, txid: bitcoin::Txid) -> Result<TxStatus, Self::Error>;
}

impl ResolveTxStatus for ExplorerResolver {
    type Error = ResolverTxStatusError;

    fn resolve_tx_status(&mut self, txid: bitcoin::Txid) -> Result<TxStatus, Self::Error> {
        if let Some(status) = self.txs_status.get(&txid) {
            Ok(status.clone())
        } else {
            Err(ResolverTxStatusError::Unknown)
        }
    }
}
