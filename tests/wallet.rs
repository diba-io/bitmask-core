#![cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use bitmask_core::{
    get_encrypted_wallet, get_network, get_wallet_data, save_mnemonic_seed, util::init_logging,
    warn,
};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn error_for_bad_mnemonic() -> Result<()> {
    init_logging("wallet=warn");

    let network = get_network()?;
    info!("Wallet test on {network}");

    info!("Import wallets");
    let mnemonic = "this is a bad mnemonic that is meant to break";
    let mnemonic_data_result = save_mnemonic_seed(mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD);

    assert!(mnemonic_data_result.is_err());

    Ok(())
}

#[tokio::test]
async fn create_wallet() -> Result<()> {
    init_logging("wallet=warn");

    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let main_mnemonic_data =
        save_mnemonic_seed(&main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_encrypted_wallet(
        ENCRYPTION_PASSWORD,
        &main_mnemonic_data.serialized_encrypted_message,
    )?;

    let main_btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;

    warn!("Descriptor:", main_vault.private.btc_descriptor_xprv);
    warn!("Address:", main_btc_wallet.address);

    Ok(())
}

#[tokio::test]
async fn get_wallet_balance() -> Result<()> {
    init_logging("wallet=warn");

    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let main_mnemonic_data =
        save_mnemonic_seed(&main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_encrypted_wallet(
        ENCRYPTION_PASSWORD,
        &main_mnemonic_data.serialized_encrypted_message,
    )?;

    let main_btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;

    let btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;
    warn!("Descriptor:", main_vault.private.btc_descriptor_xprv);
    warn!("Address:", main_btc_wallet.address);
    warn!("Wallet Balance:", btc_wallet.balance.confirmed.to_string());

    Ok(())
}
