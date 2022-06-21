use std::rc::Rc;

use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SyncOptions, Wallet};
use bdk_macros::{maybe_async, maybe_await};

use crate::data::{
    constants::{BITCOIN_EXPLORER_API, NETWORK},
    structs::ThinAsset,
};

#[allow(dead_code)] // TODO: Should this code be used?
#[derive(Default, Clone)]
struct State {
    wallet: Rc<Option<Wallet<MemoryDatabase>>>,
    rgb_assets: Option<Vec<ThinAsset>>,
    address: String,
    balance: String,
}

pub async fn get_wallet(
    descriptor: String,
    change_descriptor: Option<String>,
) -> Result<Wallet<MemoryDatabase>> {
    let wallet = Wallet::new(
        &descriptor,
        change_descriptor.as_ref(),
        *NETWORK.read().unwrap(),
        MemoryDatabase::default(),
    )?;
    maybe_await!(synchronize_wallet(&wallet))?;
    Ok(wallet)
}

pub fn get_blockchain() -> EsploraBlockchain {
    EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().unwrap(), 100)
}

#[maybe_async]
pub fn synchronize_wallet(wallet: &Wallet<MemoryDatabase>) -> Result<()> {
    let blockchain = get_blockchain();
    maybe_await!(wallet.sync(&blockchain, SyncOptions::default()))?;
    Ok(())
}
