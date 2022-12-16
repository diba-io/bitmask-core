#![cfg(not(target_arch = "wasm32"))]

use std::env;

use anyhow::Result;
use bitmask_core::{get_encrypted_wallet, get_network, save_mnemonic_seed, send_sats};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn payjoin() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var(
            "RUST_LOG",
            "bitmask_core=debug,bitmask_core::operations::rgb=trace,asset=debug",
        );
    }

    pretty_env_logger::init();

    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");
    let mnemonic_data = save_mnemonic_seed(mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let vault = get_encrypted_wallet(
        ENCRYPTION_PASSWORD,
        &mnemonic_data.serialized_encrypted_message,
    )?;

    info!("Initiating PayJoin using BIP-21");
    let destination = "bitcoin:tb1pmp4d7ksutxymw8prnwy7mvd8g4q2zltq4qd7t4h8xn8ncuz80lessgcwma?pj=https://testnet.demo.btcpayserver.org/BTC/pj";
    let amount = 1000;

    send_sats(
        &vault.btc_descriptor_xprv,
        &vault.btc_change_descriptor_xprv,
        destination,
        amount,
        Some(1.1),
    )
    .await?;

    Ok(())
}
