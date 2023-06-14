use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SyncOptions, Wallet};
use bitcoin::Network;
use futures::Future;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, RwLock};

use crate::{
    constants::{BITCOIN_EXPLORER_API, NETWORK},
    debug,
};

pub type MemoryWallet = Arc<Mutex<Wallet<MemoryDatabase>>>;
type Wallets = BTreeMap<(String, Option<String>), MemoryWallet>;
type NetworkWallet = Arc<RwLock<Wallets>>;

#[derive(Default)]
struct Networks {
    bitcoin: NetworkWallet,
    testnet: NetworkWallet,
    signet: NetworkWallet,
    regtest: NetworkWallet,
}

static BDK: Lazy<Networks> = Lazy::new(Networks::default);

async fn access_network_wallets<U, F, Fut>(network: Network, mut f: F) -> Result<()>
where
    U: 'static + Send,
    F: 'static + FnMut(NetworkWallet) -> Fut + Send,
    Fut: 'static + Future<Output = Result<U>> + Send,
{
    match network {
        Network::Bitcoin => {
            f(BDK.bitcoin.clone()).await?;
        }
        Network::Testnet => {
            f(BDK.testnet.clone()).await?;
        }
        Network::Signet => {
            f(BDK.signet.clone()).await?;
        }
        Network::Regtest => {
            f(BDK.regtest.clone()).await?;
        }
    };

    Ok(())
}

pub async fn get_wallet(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<MemoryWallet> {
    let descriptor = descriptor.to_owned();
    let key = (descriptor.clone(), change_descriptor.clone());

    let network_lock = NETWORK.read().await;
    let network = network_lock.to_owned();
    drop(network_lock);

    let wallets = match network {
        Network::Bitcoin => BDK.bitcoin.clone(),
        Network::Testnet => BDK.testnet.clone(),
        Network::Signet => BDK.signet.clone(),
        Network::Regtest => BDK.regtest.clone(),
    };

    let wallets = wallets.clone();
    let wallets_lock = wallets.read().await;
    let wallets_ref = wallets_lock.get(&key);
    if let Some(wallets) = wallets_ref {
        return Ok(wallets.clone());
    }
    drop(wallets_lock);

    let new_wallet = Arc::new(Mutex::new(Wallet::new(
        &descriptor,
        change_descriptor.as_ref(),
        network,
        MemoryDatabase::default(),
    )?));

    let key_outer = key.clone();
    let new_wallet_outer = new_wallet.clone();

    access_network_wallets(network, move |wallets| {
        let key_inner = key_outer.clone();
        let new_wallet_inner = new_wallet_outer.clone();

        async move {
            wallets.write().await.insert(key_inner, new_wallet_inner);
            Ok(())
        }
    })
    .await?;

    Ok(new_wallet)
}

pub async fn get_blockchain() -> EsploraBlockchain {
    debug!("Getting blockchain");
    EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().await, 100)
}

pub async fn sync_wallet(wallet: &MemoryWallet) -> Result<()> {
    let blockchain = get_blockchain().await;
    wallet
        .lock()
        .await
        .sync(&blockchain, SyncOptions::default())
        .await?;

    debug!("Wallet synced");
    Ok(())
}

pub async fn sync_wallets() -> Result<()> {
    let network_lock = NETWORK.read().await;
    let network = network_lock.to_owned();
    drop(network_lock);

    /* // BDK RefCell prevents this from working:
       access_network_wallets(network, move |wallets| async move {
           for (key, &mut wallet) in wallets.write().await.iter_mut() {
               let blockchain = get_blockchain().await;
               let wallet = wallet.lock().await;
               let wallet_sync_fut = wallet.sync(&blockchain, SyncOptions::default());
               wallet_sync_fut.await?;
           }

           Ok(())
       });
    */

    match network {
        Network::Bitcoin => {
            let wallets = BDK.bitcoin.clone();
            for (_key, wallet) in wallets.write().await.iter_mut() {
                let blockchain = get_blockchain().await;
                let wallet = wallet.lock().await;
                let wallet_sync_fut = wallet.sync(&blockchain, SyncOptions::default());
                wallet_sync_fut.await?;
            }
        }
        Network::Testnet => {
            let wallets = BDK.testnet.clone();
            for (_key, wallet) in wallets.write().await.iter_mut() {
                let blockchain = get_blockchain().await;
                let wallet = wallet.lock().await;
                let wallet_sync_fut = wallet.sync(&blockchain, SyncOptions::default());
                wallet_sync_fut.await?;
            }
        }
        Network::Signet => {
            let wallets = BDK.signet.clone();
            for (_key, wallet) in wallets.write().await.iter_mut() {
                let blockchain = get_blockchain().await;
                let wallet = wallet.lock().await;
                let wallet_sync_fut = wallet.sync(&blockchain, SyncOptions::default());
                wallet_sync_fut.await?;
            }
        }
        Network::Regtest => {
            let wallets = BDK.regtest.clone();
            for (_key, wallet) in wallets.write().await.iter_mut() {
                let blockchain = get_blockchain().await;
                let wallet = wallet.lock().await;
                let wallet_sync_fut = wallet.sync(&blockchain, SyncOptions::default());
                wallet_sync_fut.await?;
            }
        }
    };

    debug!("All wallets synced");
    Ok(())
}
