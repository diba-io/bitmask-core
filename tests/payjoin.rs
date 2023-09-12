#![cfg(not(target_arch = "wasm32"))]

use std::env;

use anyhow::Result;
use bitmask_core::{
    constants::switch_network, decrypt_wallet, encrypt_wallet, get_wallet_data, hash_password,
    send_sats, structs::SecretString, util::init_logging,
};
use log::{debug, info};

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[ignore]
#[tokio::test]
async fn payjoin() -> Result<()> {
    init_logging("payjoin=warn");

    switch_network("testnet").await?;

    info!("Import wallets");
    let mnemonic = env::var("TEST_WALLET_SEED")?;
    hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()));
    let encrypted_descriptors = encrypt_wallet(
        &SecretString(mnemonic),
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await?;

    let vault = decrypt_wallet(&encrypted_descriptors)?;

    let wallet = get_wallet_data(
        &SecretString(vault.private.btc_descriptor_xprv.to_string()),
        Some(&SecretString(
            vault.private.btc_change_descriptor_xprv.to_string(),
        )),
    )
    .await?;
    info!("Address: {}", wallet.address);

    info!("Initiating PayJoin using BIP-21");
    let address = wallet.address;
    let destination = format!("bitcoin:{address}?pj=https://testnet.demo.btcpayserver.org/BTC/pj");
    let amount = 1000;

    match send_sats(
        &SecretString(vault.private.btc_descriptor_xprv.to_string()),
        &SecretString(vault.private.btc_change_descriptor_xprv.to_string()),
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
