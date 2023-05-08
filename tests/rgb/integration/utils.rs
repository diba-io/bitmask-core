#![cfg(not(target_arch = "wasm32"))]
use std::{env, process::Stdio};

use bitmask_core::bitcoin::get_wallet_data;
use tokio::process::Command;

pub const ISSUER_MNEMONIC: &str =
    "ordinary crucial edit settle pencil lion appear unlock left fly century license";

#[allow(dead_code)]
pub const OWNER_MNEMONIC: &str =
    "apology pull visa moon retreat spell elite extend secret region fly diary";

#[allow(dead_code)]
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

pub async fn send_some_coins(address: &str, amount: &str) {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/send_coins.sh", path);
    Command::new("bash")
        .arg(full_file)
        .args(&[address, amount])
        .stdout(Stdio::null())
        // .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

#[allow(dead_code)]
pub async fn stop_node() {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/stop_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        // .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

pub async fn setup_regtest(force: bool, mnemonic: Option<&str>) {
    if force {
        // Restart Nodes
        start_node().await;
    }
    match mnemonic {
        Some(words) => {
            let seed_password = "";
            let vault_data = bitmask_core::bitcoin::save_mnemonic(words, seed_password)
                .await
                .expect("invalid mnemonic");

            // Send Coins to RGB Wallet
            let fungible_snapshot =
                get_wallet_data(&vault_data.public.rgb_assets_descriptor_xpub, None)
                    .await
                    .expect("invalid wallet snapshot");
            send_some_coins(&fungible_snapshot.address, "0.1").await;
        }
        _ => {}
    };
}

#[allow(dead_code)]
pub async fn shutdown_regtest(force: bool) -> anyhow::Result<()> {
    if force {
        // Destroy Nodes
        stop_node().await;
    }
    Ok(())
}
