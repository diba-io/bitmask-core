#![cfg(not(target_arch = "wasm32"))]

use std::env;

use anyhow::Result;
use bitmask_core::{
    get_encrypted_wallet, get_wallet_data, save_mnemonic_seed, send_sats, switch_network,
};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn payjoin() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var(
            "RUST_LOG",
            "bitmask_core=debug,bitmask_core::operations::rgb=trace,payjoin=debug",
        );
    }

    pretty_env_logger::init();

    switch_network("testnet").await?;

    info!("Import wallets");
    let mnemonic = env::var("TEST_WALLET_SEED")?;
    let mnemonic_data = save_mnemonic_seed(&mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let vault = get_encrypted_wallet(
        ENCRYPTION_PASSWORD,
        &mnemonic_data.serialized_encrypted_message,
    )?;

    let wallet = get_wallet_data(
        &vault.btc_descriptor_xprv,
        Some(vault.btc_change_descriptor_xprv.clone()),
    )
    .await?;
    info!("Address: {}", wallet.address);

    info!("Initiating PayJoin using BIP-21");
    let address = wallet.address;
    let destination = format!("bitcoin:{address}?pj=https://testnet.demo.btcpayserver.org/BTC/pj");
    let amount = 1000;

    match send_sats(
        &vault.btc_descriptor_xprv,
        &vault.btc_change_descriptor_xprv,
        &destination,
        amount,
        Some(1.1),
    )
    .await
    {
        Ok(_) => {
            panic!("Unexpected");
        }
        Err(e) => {
            assert_eq!(e.to_string(), "couldn't decode PSBT");
        }
    };

    Ok(())
}
