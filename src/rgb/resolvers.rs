use std::{convert::Infallible, str::FromStr};

use amplify::hex::ToHex;
use bdk::blockchain::EsploraBlockchain;
use bp::{LockTime, Outpoint, Tx, TxIn, TxOut, Txid, VarIntArray};
use futures::executor;
use rgbstd::{resolvers::ResolveHeight, validation::ResolveTx as ResolveCommiment};
use wallet::onchain::{ResolveTx, TxResolverError};

pub struct ExplorerResolver {
    pub explorer_url: String,
}

impl ResolveTx for ExplorerResolver {
    fn resolve_tx(
        &self,
        txid: bitcoin::Txid,
    ) -> Result<bitcoin::Transaction, wallet::onchain::TxResolverError> {
        let explorer_client = EsploraBlockchain::new(&self.explorer_url, 100);

        // TODO: Review that!
        match executor::block_on(explorer_client.get_tx(&txid)).expect("service unavaliable") {
            Some(tx) => Ok(tx),
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
    fn resolve_tx(&self, txid: Txid) -> Result<Tx, rgbstd::validation::TxResolverError> {
        let explorer_client = EsploraBlockchain::new(&self.explorer_url, 100);

        let transaction_id = &bitcoin::Txid::from_str(&txid.to_hex()).expect("");
        let tx = executor::block_on(explorer_client.get_tx(transaction_id))
            .expect("service unavaliable")
            .unwrap();
        Ok(Tx {
            version: (tx.version as u8)
                .try_into()
                .expect("non-consensus tx version"),
            inputs: VarIntArray::try_from_iter(tx.input.into_iter().map(|txin| TxIn {
                prev_output: Outpoint::new(
                    Txid::from_str(&txin.previous_output.txid.to_hex()).expect(""),
                    txin.previous_output.vout,
                ),
                sig_script: txin.script_sig.to_bytes().into(),
                sequence: txin.sequence.0.into(),
            }))
            .expect("consensus-invalid transaction"),
            outputs: VarIntArray::try_from_iter(tx.output.into_iter().map(|txout| TxOut {
                value: txout.value.into(),
                script_pubkey: txout.script_pubkey.to_bytes().into(),
            }))
            .expect("consensus-invalid transaction"),
            lock_time: LockTime::from(tx.lock_time.0),
        })
    }
}
