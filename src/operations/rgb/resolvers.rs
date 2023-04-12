use bdk::blockchain::EsploraBlockchain;
use futures::executor;
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
            _ => Err(TxResolverError {
                txid: txid,
                err: none!(),
            }),
        }
    }
}
