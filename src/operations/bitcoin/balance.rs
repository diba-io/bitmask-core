use anyhow::Result;
use bdk::{
    blockchain::esplora::EsploraBlockchain,
    database::{AnyDatabase, MemoryDatabase},
    SyncOptions, Wallet,
};

use crate::{
    data::constants::{BITCOIN_EXPLORER_API, NETWORK},
    debug,
};

pub fn get_wallet(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<Wallet<AnyDatabase>> {
    // #[cfg(feature = "server")]
    // #[cfg(not(target_arch = "wasm32"))]
    // let db = {
    //     use bdk::database::SqliteDatabase;
    //     use bitcoin_hashes::{sha256, Hash, HashEngine};
    //     use std::fs;

    //     use directories::ProjectDirs;
    //     let mut engine = sha256::Hash::engine();
    //     engine.input(descriptor.as_bytes());
    //     if let Some(change_descriptor) = change_descriptor {
    //         engine.input(change_descriptor.as_bytes());
    //     };
    //     let hash = sha256::Hash::from_engine(engine);
    //     debug!("Descriptor hash:", hash.to_string());
    //     let project_dirs = ProjectDirs::from("org", "DIBA", "BitMask").unwrap();
    //     let db_path = project_dirs.data_local_dir().join("wallet_db");
    //     fs::create_dir_all(&db_path).unwrap();
    //     let db = SqliteDatabase::new(&db_path.join(hash.to_string()));
    //     AnyDatabase::Sqlite(db)
    // };
    #[cfg(not(target_arch = "wasm32"))]
    let db = AnyDatabase::Memory(MemoryDatabase::default());

    #[cfg(target_arch = "wasm32")]
    let db = AnyDatabase::Memory(MemoryDatabase::default());

    debug!(format!("Using database: {db:?}"));

    let wallet = Wallet::new(
        descriptor,
        change_descriptor.as_deref(),
        *NETWORK.read().unwrap(),
        db,
    )?;
    debug!(format!("Using wallet: {wallet:?}"));

    Ok(wallet)
}

pub fn get_blockchain() -> EsploraBlockchain {
    debug!("Getting blockchain");
    EsploraBlockchain::new(&BITCOIN_EXPLORER_API.read().unwrap(), 100)
}

pub async fn synchronize_wallet(wallet: &Wallet<AnyDatabase>) -> Result<()> {
    let blockchain = get_blockchain();
    wallet.sync(&blockchain, SyncOptions::default()).await?;
    debug!("Synced");
    Ok(())
}
