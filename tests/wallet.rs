#![cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use bitmask_core::{
    bitcoin::{encrypt_seed, get_encrypted_wallet, get_wallet_data, hash_password, new_wallet},
    constants::{get_network, switch_network},
    structs::SecretString,
    util::init_logging,
    warn,
};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn error_for_bad_mnemonic() -> Result<()> {
    init_logging("wallet=info");

    let network = get_network().await;
    info!("Wallet test on {network}");

    info!("Import wallets");
    let mnemonic = "this is a bad mnemonic that is meant to break";
    let hash = hash_password(SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let mnemonic_data_result =
        encrypt_seed(&SecretString(mnemonic), &hash, &SecretString(SEED_PASSWORD)).await;

    assert!(mnemonic_data_result.is_err());

    Ok(())
}

#[tokio::test]
async fn create_wallet() -> Result<()> {
    init_logging("wallet=info");

    switch_network("bitcoin").await?;
    let network = get_network().await;
    info!("Asset test on {network}");

    info!("Create wallet");
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD));
    let main_mnemonic = new_wallet(&hash, &SecretString(SEED_PASSWORD)).await?;
    info!("Generated mnemonic: {}", main_mnemonic.mnemonic);
    let encrypted_descriptors = encrypt_seed(&main_mnemonic.mnemonic, &hash, SEED_PASSWORD).await?;
    let main_vault = get_encrypted_wallet(&hash, &encrypted_descriptors)?;

    let main_btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;
    // let main_rgb_wallet =
    //     get_wallet_data(&main_vault.private.rgb_assets_descriptor_xprv, None).await?;

    println!("Descriptor: {}", main_vault.private.btc_descriptor_xprv);
    println!("Address (Bitcoin): {}", main_btc_wallet.address);
    // println!("Address (RGB): {}", main_rgb_wallet.address);

    Ok(())
}

#[tokio::test]
async fn import_wallet() -> Result<()> {
    init_logging("wallet=info");

    let network = get_network().await;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let hash0 = hash_password(ENCRYPTION_PASSWORD);
    let main_mnemonic_data = encrypt_seed(&main_mnemonic, &hash0, SEED_PASSWORD).await?;
    let _main_vault = get_encrypted_wallet(&hash0, &main_mnemonic_data.encrypted_descriptors)?;

    info!("Try once more");
    let hash1 = hash_password(ENCRYPTION_PASSWORD);
    assert_eq!(hash0, hash1, "hashes match");

    let main_mnemonic_data: bitmask_core::structs::MnemonicSeedData =
        encrypt_seed(&main_mnemonic, &hash1, SEED_PASSWORD).await?;
    let main_vault = get_encrypted_wallet(&hash1, &main_mnemonic_data.encrypted_descriptors)?;

    let main_btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;
    let main_rgb_wallet =
        get_wallet_data(&main_vault.private.rgb_assets_descriptor_xprv, None).await?;

    println!("Descriptor: {}", main_vault.private.btc_descriptor_xprv);
    println!("Address (Bitcoin): {}", main_btc_wallet.address);
    println!("Address (RGB): {}", main_rgb_wallet.address);

    Ok(())
}

#[tokio::test]
async fn get_wallet_balance() -> Result<()> {
    init_logging("wallet=info");

    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let hash = hash_password(ENCRYPTION_PASSWORD);
    let main_mnemonic_data = encrypt_seed(&main_mnemonic, &hash, SEED_PASSWORD).await?;
    let main_vault = get_encrypted_wallet(&hash, &main_mnemonic_data.encrypted_descriptors)?;

    let main_btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;

    let btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv, None).await?;
    warn!("Descriptor:", main_vault.private.btc_descriptor_xprv);
    warn!("Address:", main_btc_wallet.address);
    warn!("Wallet Balance:", btc_wallet.balance.confirmed.to_string());

    Ok(())
}
