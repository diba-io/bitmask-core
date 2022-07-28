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

pub fn get_wallet(
    descriptor: &str,
    change_descriptor: Option<&str>,
) -> Result<Wallet<MemoryDatabase>> {
    // #[cfg(not(target_arch = "wasm32"))]
    // let db = {
    //     use directories::ProjectDirs;
    //     use regex::Regex;

    //     let re = Regex::new(r"\(\[([0-9a-f]+)/(.+)](.+)/").unwrap();
    //     let cap = re.captures(descriptor).unwrap();
    //     let fingerprint = &cap[1];
    //     let fingerprint = if let Some(change_descriptor) = change_descriptor {
    //         let cap = re.captures(change_descriptor).unwrap();
    //         [fingerprint, &cap[1]].join("_")
    //     } else {
    //         fingerprint.to_owned()
    //     };
    //     let project_dirs = ProjectDirs::from("org", "DIBA", "BitMask").unwrap();
    //     let db: sled::Db = sled::open(project_dirs.data_local_dir().join("wallet_db"))?;
    //     db.open_tree(fingerprint).unwrap()
    // };

    // #[cfg(target_arch = "wasm32")]
    let db = MemoryDatabase::default();

    let wallet = Wallet::new(descriptor, change_descriptor, *NETWORK.read().unwrap(), db)?;
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
