use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::AnyDatabase, SyncOptions, Wallet};
use bitcoin_hashes::{sha256, Hash, HashEngine};

use crate::{
    data::constants::{BITCOIN_EXPLORER_API, NETWORK},
    debug,
};

pub fn get_wallet(
    descriptor: &str,
    change_descriptor: Option<&str>,
) -> Result<Wallet<AnyDatabase>> {
    #[cfg(not(target_arch = "wasm32"))]
    let db = {
        use directories::ProjectDirs;
        let mut engine = sha256::Hash::engine();
        engine.input(descriptor.as_bytes());
        if let Some(change_descriptor) = change_descriptor {
            engine.input(change_descriptor.as_bytes());
        };
        let hash = sha256::Hash::from_engine(engine);
        debug!("Descriptor hash:", hash.to_string());
        let project_dirs = ProjectDirs::from("org", "DIBA", "BitMask").unwrap();
        let db: sled::Db = sled::open(project_dirs.data_local_dir().join("wallet_db"))?;
        AnyDatabase::Sled(db.open_tree(hash).unwrap())
    };

    #[cfg(target_arch = "wasm32")]
    let db = MemoryDatabase::default();

    let wallet = Wallet::new(descriptor, change_descriptor, *NETWORK.read().unwrap(), db)?;
    Ok(wallet)
}

pub fn get_blockchain() -> EsploraBlockchain {
    EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().unwrap(), 100)
}

pub async fn synchronize_wallet(wallet: &Wallet<AnyDatabase>) -> Result<()> {
    let blockchain = get_blockchain();
    wallet.sync(&blockchain, SyncOptions::default()).await?;
    Ok(())
}
