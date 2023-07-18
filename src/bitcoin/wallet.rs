use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicUsize, Arc, Ordering},
};

use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SyncOptions, Wallet};
use bitcoin::Network;
use bitcoin_hashes::{sha256, Hash};
use futures::Future;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, RwLock};

use crate::{
    constants::{BITCOIN_EXPLORER_API, NETWORK},
    debug,
    structs::SecretString,
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
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
) -> Result<Arc<Mutex<Wallet<MemoryDatabase>>>> {
    let descriptor_key = format!("{descriptor:?}{change_descriptor:?}");
    let key = sha256::Hash::hash(descriptor_key.as_bytes()).to_string();

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
    let wallets_ref = wallets_lock.get(&(key.clone(), None));
    if let Some(wallets) = wallets_ref {
        return Ok(wallets.clone());
    }
    drop(wallets_lock);

    let new_wallet = Arc::new(Mutex::new(Wallet::new(
        &descriptor.0,
        change_descriptor.map(|desc| &desc.0),
        network,
        MemoryDatabase::default(),
    )?));

    let key_outer = key;
    let new_wallet_outer = new_wallet.clone();

    access_network_wallets(network, move |wallets| {
        let key_inner = key_outer.clone();
        let new_wallet_inner = new_wallet_outer.clone();

        async move {
            wallets
                .write()
                .await
                .insert((key_inner, None), new_wallet_inner);
            Ok(())
        }
    })
    .await?;

    Ok(new_wallet)
}

// pub async fn get_blockchain() -> EsploraBlockchain {
//     debug!("Getting blockchain");
//     EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().await, 100)
// }

pub async fn sync_wallet(wallet: &MemoryWallet) -> Result<()> {
    retry_blockchain(|blockchain| async move {
        wallet
            .lock()
            .await
            .sync(&blockchain, SyncOptions::default())
            .await?;
        Ok(())
    })
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
            retry_blockchain(|blockchain| async move {
                let wallets = BDK.bitcoin.clone();
                for (_key, wallet) in wallets.write().await.iter_mut() {
                    wallet
                        .lock()
                        .await
                        .sync(&blockchain, SyncOptions::default())
                        .await?;
                }
                Ok(())
            })
            .await?
        }
        Network::Testnet => {
            retry_blockchain(|blockchain| async move {
                let wallets = BDK.testnet.clone();
                for (_key, wallet) in wallets.write().await.iter_mut() {
                    wallet
                        .lock()
                        .await
                        .sync(&blockchain, SyncOptions::default())
                        .await?;
                }
                Ok(())
            })
            .await?
        }
        Network::Signet => {
            retry_blockchain(|blockchain| async move {
                let wallets = BDK.signet.clone();
                for (_key, wallet) in wallets.write().await.iter_mut() {
                    wallet
                        .lock()
                        .await
                        .sync(&blockchain, SyncOptions::default())
                        .await?;
                }
                Ok(())
            })
            .await?
        }
        Network::Regtest => {
            retry_blockchain(|blockchain| async move {
                let wallets = BDK.regtest.clone();
                for (_key, wallet) in wallets.write().await.iter_mut() {
                    wallet
                        .lock()
                        .await
                        .sync(&blockchain, SyncOptions::default())
                        .await?;
                }
                Ok(())
            })
            .await?
        }
    };

    debug!("All wallets synced");
    Ok(())
}

static EXPLORER_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

async fn retry_blockchain<U, F, Fut>(mut f: F) -> Result<()>
where
    U: 'static + Send,
    F: 'static + FnMut(EsploraBlockchain) -> Fut + Send,
    Fut: 'static + Future<Output = Result<U>> + Send,
{
    loop {
        let explorer_urls = BITCOIN_EXPLORER_API.read().await.split(",");
        let explorer_count = explorer_urls.count();
        let explorer_index = EXPLORER_INDEX.load(Ordering::SeqCst);
        let base_urls: Vec<String> = explorer_urls.map(str::to_string).collect();
        let base_url = base_urls
            .get(explorer_index)
            .expect("Index within bounds of available explorers");

        debug!(format!("Using explorer URL: {}", &base_url));
        let blockchain = EsploraBlockchain::new(&base_url, 5);

        match f(blockchain).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if e.to_string().contains("Esplora") {
                    // Increment explorer_index upon Esplora errors
                    let explorer_index = EXPLORER_INDEX.load(Ordering::SeqCst);
                    EXPLORER_INDEX.store((explorer_index + 1) % explorer_count, Ordering::SeqCst);
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
}
