use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SyncOptions, Wallet};
use bitcoin_hashes::{sha256, Hash};
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, RwLock};

use crate::{
    constants::{BITCOIN_EXPLORER_API, NETWORK},
    debug,
    structs::SecretString,
};

pub type MemoryWallet = Arc<Mutex<Wallet<MemoryDatabase>>>;
type Wallets = BTreeMap<String, MemoryWallet>;

#[derive(Default)]
struct Networks {
    bitcoin: Arc<RwLock<Wallets>>,
    testnet: Arc<RwLock<Wallets>>,
    signet: Arc<RwLock<Wallets>>,
    regtest: Arc<RwLock<Wallets>>,
}

static BDK: Lazy<Networks> = Lazy::new(Networks::default);

pub async fn get_wallet(
    descriptor: &SecretString,
    change_descriptor: Option<SecretString>,
) -> Result<Arc<Mutex<Wallet<MemoryDatabase>>>> {
    let descriptor_key = format!("{descriptor:?}{change_descriptor:?}");
    let key = sha256::Hash::hash(descriptor_key.as_bytes()).to_string();

    let network_lock = NETWORK.read().await;
    let network = network_lock.to_owned();
    drop(network_lock);

    let wallets = match network {
        bitcoin::Network::Bitcoin => BDK.bitcoin.clone(),
        bitcoin::Network::Testnet => BDK.testnet.clone(),
        bitcoin::Network::Signet => BDK.signet.clone(),
        bitcoin::Network::Regtest => BDK.regtest.clone(),
    };

    let wallets = wallets.clone();
    let wallets_lock = wallets.read().await;
    let wallets_ref = wallets_lock.get(&key);
    if let Some(wallets) = wallets_ref {
        return Ok(wallets.clone());
    }
    drop(wallets_lock);

    let mut change_descriptor = None;
    if let Some(desc) = change_descriptor {
        change_descriptor = Some(desc);
    };

    let new_wallet = Arc::new(Mutex::new(Wallet::new(
        &descriptor.0,
        change_descriptor,
        network,
        MemoryDatabase::default(),
    )?));

    match network {
        bitcoin::Network::Bitcoin => {
            BDK.bitcoin.write().await.insert(key, new_wallet.clone());
        }
        bitcoin::Network::Testnet => {
            BDK.testnet.write().await.insert(key, new_wallet.clone());
        }
        bitcoin::Network::Signet => {
            BDK.signet.write().await.insert(key, new_wallet.clone());
        }
        bitcoin::Network::Regtest => {
            BDK.regtest.write().await.insert(key, new_wallet.clone());
        }
    };

    Ok(new_wallet)
}

pub async fn get_blockchain() -> EsploraBlockchain {
    debug!("Getting blockchain");
    EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().await, 100)
}

pub async fn synchronize_wallet(wallet: &MemoryWallet) -> Result<()> {
    let blockchain = get_blockchain().await;
    wallet
        .lock()
        .await
        .sync(&blockchain, SyncOptions::default())
        .await?;
    debug!("Synced");
    Ok(())
}
