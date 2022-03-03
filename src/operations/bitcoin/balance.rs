use std::rc::Rc;

use anyhow::Result;
use bdk::{
    blockchain::{esplora::EsploraBlockchain, noop_progress},
    database::MemoryDatabase,
    Wallet,
};

use crate::data::{
    constants::{BITCOIN_EXPLORER_API, NETWORK},
    structs::ThinAsset,
};

#[allow(dead_code)] // TODO: Should this code be used?
#[derive(Default, Clone)]
struct State {
    wallet: Rc<Option<Wallet<EsploraBlockchain, MemoryDatabase>>>,
    rgb_assets: Option<Vec<ThinAsset>>,
    address: String,
    balance: String,
}

pub async fn get_wallet(
    descriptor: String,
    change_descriptor: String,
) -> Result<Wallet<EsploraBlockchain, MemoryDatabase>> {
    let blockchain = EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().unwrap(), 20);
    let wallet = Wallet::new(
        &descriptor,
        Some(&change_descriptor),
        *NETWORK.read().unwrap(),
        MemoryDatabase::default(),
        blockchain,
    )
    .await?;
    synchronize_wallet(&wallet).await?;
    Ok(wallet)
}

pub async fn synchronize_wallet(wallet: &Wallet<EsploraBlockchain, MemoryDatabase>) -> Result<()> {
    wallet.sync(noop_progress(), None).await?;
    Ok(())
}
