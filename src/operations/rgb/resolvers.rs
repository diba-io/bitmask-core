use std::convert::Infallible;

use bdk::blockchain::EsploraBlockchain;
use futures::executor;
use rgbstd::resolvers::ResolveHeight;
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
