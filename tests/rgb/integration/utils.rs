#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::{
    bitcoin::{get_wallet, get_wallet_data, synchronize_wallet},
    structs::EncryptedWalletData,
};
use std::{env, process::Stdio};
use tokio::process::Command;

pub const REGTEST_MNEMONIC: &str =
    "ordinary crucial edit settle pencil lion appear unlock left fly century license";

pub async fn start_node() {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/startup_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

pub async fn stop_node() {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/stop_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

pub async fn send_some_coins(address: &str, amount: &str) {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/send_coins.sh", path);
    Command::new("bash")
        .arg(full_file)
        .args(&[address, amount])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

pub async fn setup_integration() -> anyhow::Result<EncryptedWalletData> {
    if env::var("RESET_DOCKER_ENV").is_ok() {
        // Start Node
        start_node().await;
    }
    let mnemonic_phrase = REGTEST_MNEMONIC;
    let seed_password = "";
    let vault_data = bitmask_core::bitcoin::save_mnemonic(mnemonic_phrase, seed_password).await?;

    // Send Coins to RGB Wallet
    let fungible_wallet = get_wallet(&vault_data.public.rgb_assets_descriptor_xpub, None).await?;
    let fungible_snapshot =
        get_wallet_data(&vault_data.public.rgb_assets_descriptor_xpub, None).await?;

    send_some_coins(&fungible_snapshot.address, "0.01").await;
    synchronize_wallet(&fungible_wallet).await?;
    Ok(vault_data)
}

pub async fn shutdown_integration() -> anyhow::Result<()> {
    if env::var("RESET_DOCKER_ENV").is_ok() {
        // Start Node
        stop_node().await;
    }
    Ok(())
}
