#![allow(unused_imports)]
#![allow(unused_variables)]
use amplify::hex::ToHex;
use bitcoin::Script;
use bp::{LockTime, Outpoint, SeqNo, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray, Witness};
use rgb::{prelude::DeriveInfo, MiningStatus, Utxo};
use rgbstd::{resolvers::ResolveHeight, validation::ResolveTx as ResolveCommiment};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::Infallible,
    str::FromStr,
};
use wallet::onchain::{ResolveTx, TxResolverError};

#[derive(Default)]
pub struct ExplorerResolver {
    pub explorer_url: String,
    // Prefetch Data (wasm32)
    pub utxos: BTreeSet<Utxo>,
    pub next_utxo: String,
    pub txs: HashMap<bitcoin::Txid, bitcoin::Transaction>,
    pub bp_txs: HashMap<Txid, Tx>,
}

impl rgb::Resolver for ExplorerResolver {
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_utxo<'s>(
        &mut self,
        scripts: BTreeMap<DeriveInfo, bitcoin_30::ScriptBuf>,
    ) -> Result<BTreeSet<rgb::prelude::Utxo>, String> {
        let mut utxos = bset![];
        let explorer_client = esplora_block::Builder::new(&self.explorer_url)
            .build_blocking()
            .expect("service unavaliable");
        // TODO: Remove that after bitcoin v.30 full compatibility
        let script_list = scripts.into_iter().map(|(d, sc)| {
            (
                d,
                Script::from_str(&sc.to_hex_string()).expect("invalid script"),
            )
        });

        for (derive, script) in script_list {
            let txs = match explorer_client.scripthash_txs(&script, none!()) {
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
                        Txid::from_str(&tx.txid.to_hex()).expect("invalid outpoint parse"),
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
    type Error = Infallible;

    fn resolve_height(&mut self, _txid: bp::Txid) -> Result<u32, Self::Error> {
        Ok(0)
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
            .expect("service unavaliable")
            .unwrap();
        Ok(Tx {
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
        })
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
        Ok(self.next_utxo == outpoint)
    }
}
