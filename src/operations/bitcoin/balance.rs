use std::rc::Rc;

use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SyncOptions, Wallet};

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
    change_descriptor: String,
) -> Result<Wallet<MemoryDatabase>> {
    let wallet = Wallet::new(
        &descriptor,
        Some(&change_descriptor),
        *NETWORK.read().unwrap(),
        MemoryDatabase::default(),
    )?;
    synchronize_wallet(&wallet).await?;
    Ok(wallet)
}

pub fn get_blockchain() -> EsploraBlockchain {
    EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().unwrap(), 100)
}

pub async fn synchronize_wallet(wallet: &Wallet<MemoryDatabase>) -> Result<()> {
    let blockchain = get_blockchain();
    wallet.sync(&blockchain, SyncOptions::default()).await?;
    Ok(())
}
