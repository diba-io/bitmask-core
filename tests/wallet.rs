#![cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use bitmask_core::{
    constants::{get_network, switch_network},
    decrypt_wallet, encrypt_wallet, get_wallet_data, hash_password, new_wallet, send_sats,
    structs::SecretString,
    util::init_logging,
    warn, BitcoinError,
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
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let mnemonic_data_result = encrypt_wallet(
        &SecretString(mnemonic.to_owned()),
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
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = new_wallet(&SecretString(SEED_PASSWORD.to_owned())).await?;
    let decrypted_wallet = decrypt_wallet(&encrypted_descriptors)?;

    let main_btc_wallet =
        get_wallet_data(&decrypted_wallet.private.btc_descriptor_xprv.clone(), None).await?;
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
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &seed_password).await?;
    let _main_vault = decrypt_wallet(&encrypted_descriptors)?;

    info!("Try once more");
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));

    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &seed_password).await?;
    let main_vault = decrypt_wallet(&encrypted_descriptors)?;

    let main_btc_wallet =
        get_wallet_data(&main_vault.private.btc_descriptor_xprv.clone(), None).await?;
    let main_rgb_wallet =
        get_wallet_data(&main_vault.private.rgb_assets_descriptor_xprv.clone(), None).await?;

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
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &seed_password).await?;
    let main_vault = decrypt_wallet(&encrypted_descriptors)?;

    let btc_wallet = get_wallet_data(&main_vault.private.btc_descriptor_xprv.clone(), None).await?;

    warn!(
        "Descriptor:",
        main_vault.private.btc_descriptor_xprv.to_string()
    );
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
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &seed_password).await?;

    let main_vault = decrypt_wallet(&encrypted_descriptors)?;

    let result = send_sats(
        &main_vault.private.btc_descriptor_xprv.to_owned(),
        &main_vault.private.btc_change_descriptor_xprv.to_owned(),
        "bc1pgxpvg7cz0s3akgl9vhv687rzya7frskenukgx3gwuh6q3un5wqgq7xmnhe",
        1000,
        Some(1.0),
    )
    .await;

    assert!(matches!(result, Err(BitcoinError::WrongNetwork)));

    Ok(())
}
