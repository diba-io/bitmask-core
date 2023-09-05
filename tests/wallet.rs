#![cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use bitmask_core::{
    bitcoin::{
        decrypt_wallet, encrypt_wallet, get_wallet_data, hash_password, new_wallet, send_sats,
        BitcoinError,
    },
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
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let mnemonic_data_result = encrypt_wallet(
        &SecretString(mnemonic.to_owned()),
        &hash,
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await;

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
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = new_wallet(&hash, &SecretString(SEED_PASSWORD.to_owned())).await?;
    let decrypted_wallet = decrypt_wallet(&hash, &encrypted_descriptors)?;

    let main_btc_wallet = get_wallet_data(
        &SecretString(decrypted_wallet.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;
    // let main_rgb_wallet =
    //     get_wallet_data(&decrypted_wallet.private.rgb_assets_descriptor_xprv, None).await?;

    println!(
        "Descriptor: {}",
        decrypted_wallet.private.btc_descriptor_xprv
    );
    println!("Address (Bitcoin): {}", main_btc_wallet.address);

    Ok(())
}

#[tokio::test]
async fn import_wallet() -> Result<()> {
    init_logging("wallet=info");

    let network = get_network().await;
    info!("Asset test on {network}");

    info!("Import wallets");
    let seed_password = SecretString(SEED_PASSWORD.to_owned());
    let main_mnemonic = SecretString(env::var("TEST_WALLET_SEED")?);
    let hash0 = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash0, &seed_password).await?;
    let _main_vault = decrypt_wallet(&hash0, &encrypted_descriptors)?;

    info!("Try once more");
    let hash1 = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    assert_eq!(hash0.0, hash1.0, "hashes match");

    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash1, &seed_password).await?;
    let main_vault = decrypt_wallet(&hash1, &encrypted_descriptors)?;

    let main_btc_wallet = get_wallet_data(
        &SecretString(main_vault.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;
    let main_rgb_wallet = get_wallet_data(
        &SecretString(main_vault.private.rgb_assets_descriptor_xprv.clone()),
        None,
    )
    .await?;

    println!("Descriptor: {}", main_vault.private.btc_descriptor_xprv);
    println!("Address (Bitcoin): {}", main_btc_wallet.address);
    println!("Address (RGB): {}", main_rgb_wallet.address);

    Ok(())
}

#[tokio::test]
async fn get_wallet_balance() -> Result<()> {
    init_logging("wallet=info");

    let main_mnemonic = SecretString(env::var("TEST_WALLET_SEED")?);
    let seed_password = SecretString(SEED_PASSWORD.to_owned());
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash, &seed_password).await?;
    let main_vault = decrypt_wallet(&hash, &encrypted_descriptors)?;

    let btc_wallet = get_wallet_data(
        &SecretString(main_vault.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;

    warn!("Descriptor:", main_vault.private.btc_descriptor_xprv);
    warn!("Address:", btc_wallet.address);
    warn!("Wallet Balance:", btc_wallet.balance.confirmed.to_string());

    Ok(())
}

#[tokio::test]
async fn wrong_network() -> Result<()> {
    init_logging("wallet=info");

    switch_network("testnet").await?;
    let network = get_network().await;
    info!("Asset test on {network}");

    let main_mnemonic = SecretString(env::var("TEST_WALLET_SEED")?);
    let seed_password = SecretString(SEED_PASSWORD.to_owned());
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash, &seed_password).await?;

    let main_vault = decrypt_wallet(&hash, &encrypted_descriptors)?;

    let result = send_sats(
        &SecretString(main_vault.private.btc_descriptor_xprv.to_owned()),
        &SecretString(main_vault.private.btc_change_descriptor_xprv.to_owned()),
        "bc1pgxpvg7cz0s3akgl9vhv687rzya7frskenukgx3gwuh6q3un5wqgq7xmnhe",
        1000,
        Some(1.0),
    )
    .await;

    assert!(matches!(result, Err(BitcoinError::WrongNetwork)));

    Ok(())
}
