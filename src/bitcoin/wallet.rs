use std::{collections::BTreeMap, sync::Arc};

use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, SyncOptions, Wallet};
use bitcoin::Network;
use bitcoin_hashes::{sha256, Hash};
use futures::Future;
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};

use crate::{
    constants::{dot_env, NETWORK},
    debug,
    structs::SecretString,
};

#[derive(Error, Debug)]
pub enum BitcoinWalletError {
    /// Unexpected key variant in get_descriptor
    #[error("Unexpected key variant in get_descriptor")]
    UnexpectedKey,
    /// BDK error
    #[error(transparent)]
    BdkError(#[from] bdk::Error),
}

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

struct Blockchains {
    bitcoin: Arc<EsploraBlockchain>,
    testnet: Arc<EsploraBlockchain>,
    signet: Arc<EsploraBlockchain>,
    regtest: Arc<EsploraBlockchain>,
}

static BDK: Lazy<Networks> = Lazy::new(Networks::default);
static BLOCKCHAINS: Lazy<Blockchains> = Lazy::new(|| {
    let bitcoin = Arc::new(EsploraBlockchain::new(
        &dot_env("BITCOIN_EXPLORER_API_MAINNET"),
        1,
    ));
    let testnet = Arc::new(EsploraBlockchain::new(
        &dot_env("BITCOIN_EXPLORER_API_TESTNET"),
        1,
    ));
    let signet = Arc::new(EsploraBlockchain::new(
        &dot_env("BITCOIN_EXPLORER_API_SIGNET"),
        1,
    ));
    let regtest = Arc::new(EsploraBlockchain::new(
        &dot_env("BITCOIN_EXPLORER_API_REGTEST"),
        1,
    ));

    Blockchains {
        bitcoin,
        testnet,
        signet,
        regtest,
    }
});

async fn access_network_wallets<U, F, Fut>(
    network: Network,
    mut f: F,
) -> Result<(), BitcoinWalletError>
where
    U: 'static + Send,
    F: 'static + FnMut(NetworkWallet) -> Fut + Send,
    Fut: 'static + Future<Output = Result<U, BitcoinWalletError>> + Send,
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
) -> Result<Arc<Mutex<Wallet<MemoryDatabase>>>, BitcoinWalletError> {
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

pub async fn get_blockchain() -> Arc<EsploraBlockchain> {
    let network_lock = NETWORK.read().await;
    let network = network_lock.to_owned();
    drop(network_lock);

    debug!("Getting blockchain");

    match network {
        Network::Bitcoin => BLOCKCHAINS.bitcoin.clone(),
        Network::Testnet => BLOCKCHAINS.testnet.clone(),
        Network::Signet => BLOCKCHAINS.signet.clone(),
        Network::Regtest => BLOCKCHAINS.regtest.clone(),
    }
}

pub async fn sync_wallet(wallet: &MemoryWallet) -> Result<(), BitcoinWalletError> {
    let blockchain = get_blockchain().await;
    wallet
        .lock()
        .await
        .sync(&blockchain, SyncOptions::default())
        .await?;

    debug!("Wallet synced");
    Ok(())
}

pub async fn sync_wallets() -> Result<(), BitcoinWalletError> {
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
