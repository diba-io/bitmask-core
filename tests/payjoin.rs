#![cfg(not(target_arch = "wasm32"))]

use std::env;

use anyhow::Result;
use bitmask_core::{
    get_encrypted_wallet, get_wallet_data, save_mnemonic_seed, send_sats, switch_network,
    util::init_logging,
};
use log::{debug, info};

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn payjoin() -> Result<()> {
    init_logging("payjoin=warn");

    switch_network("testnet").await?;

    info!("Import wallets");
    let mnemonic = env::var("TEST_WALLET_SEED")?;
    let mnemonic_data = save_mnemonic_seed(&mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD).await?;

    let vault = get_encrypted_wallet(
        ENCRYPTION_PASSWORD,
        &mnemonic_data.serialized_encrypted_message,
    )?;

    let wallet = get_wallet_data(
        &vault.private.btc_descriptor_xprv,
        Some(vault.private.btc_change_descriptor_xprv.clone()),
    )
    .await?;
    info!("Address: {}", wallet.address);

    info!("Initiating PayJoin using BIP-21");
    let address = wallet.address;
    let destination = format!("bitcoin:{address}?pj=https://testnet.demo.btcpayserver.org/BTC/pj");
    let amount = 1000;

    match send_sats(
        &vault.private.btc_descriptor_xprv,
        &vault.private.btc_change_descriptor_xprv,
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
            debug!("{:#?}", e);
            assert!(e.to_string().contains("invoice-not-found"));
        }
    };

    Ok(())
}
