#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::{get_network, save_mnemonic_seed};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn error_for_bad_mnemonic() -> Result<()> {
    let _ = pretty_env_logger::try_init();
    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let mnemonic = "this is a bad mnemonic that is meant to break";
    let mnemonic_data_result = save_mnemonic_seed(mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD);

    assert!(mnemonic_data_result.is_err());

    Ok(())
}
