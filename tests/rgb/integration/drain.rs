#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::{
    bitcoin::{drain_wallet, get_wallet_data, new_mnemonic, save_mnemonic},
    structs::SecretString,
};

use crate::rgb::integration::utils::{send_some_coins, OWNER_MNEMONIC};

#[tokio::test]
pub async fn drain() -> Result<()> {
    // 1. Initial Setup
    let old_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let new_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    let old_wallet_data = get_wallet_data(
        &SecretString(old_keys.public.btc_descriptor_xpub.clone()),
        Some(&SecretString(
            old_keys.public.btc_change_descriptor_xpub.clone(),
        )),
    )
    .await?;

    send_some_coins(&old_wallet_data.address, "0.1").await;
    send_some_coins(&old_wallet_data.address, "0.1").await;
    send_some_coins(&old_wallet_data.address, "0.1").await;

    let new_wallet_data = get_wallet_data(
        &SecretString(new_keys.public.btc_descriptor_xpub.clone()),
        Some(&SecretString(
            new_keys.public.btc_change_descriptor_xpub.clone(),
        )),
    )
    .await?;

    // 2. Drain sats from original wallet to new wallet
    let drain_wallet_details = drain_wallet(
        &new_wallet_data.address,
        &SecretString(old_keys.private.btc_descriptor_xprv.clone()),
        Some(&SecretString(
            old_keys.private.btc_change_descriptor_xprv.clone(),
        )),
        Some(2.0),
    )
    .await?;

    assert_eq!(
        drain_wallet_details.received, 0,
        "received no funds in this transaction"
    );
    assert_eq!(
        drain_wallet_details.sent + drain_wallet_details.fee.expect("fee present"),
        30_000_000,
        "received 0.3 tBTC"
    );

    Ok(())
}
