#![cfg(not(target_arch = "wasm32"))]
use bdk::{database::AnyDatabase, Wallet};
use bitmask_core::{
    bitcoin::{get_wallet, get_wallet_data, synchronize_wallet},
    util::init_logging,
};

use crate::rgb::integration::utils::{send_some_coins, start_node, stop_node, REGTEST_MNEMONIC};

#[tokio::test]
async fn allow_utxo_selection() -> anyhow::Result<()> {
    init_logging("rgb_issue=warn");
    let _ = before_test().await?;
    // stop_node().await;
    Ok(())
}

async fn before_test() -> anyhow::Result<Wallet<AnyDatabase>> {
    // Start Node
    start_node().await;
    let mnemonic_phrase = REGTEST_MNEMONIC;
    let seed_password = "";
    let vault_data = bitmask_core::bitcoin::save_mnemonic(mnemonic_phrase, seed_password).await?;

    let fungible_wallet = get_wallet(&vault_data.public.btc_change_descriptor_xpub, None).await?;
    let fungible_snapshot =
        get_wallet_data(&vault_data.public.btc_change_descriptor_xpub, None).await?;

    // Send Coins
    send_some_coins(&fungible_snapshot.address, "1").await;
    synchronize_wallet(&fungible_wallet).await?;

    Ok(fungible_wallet)
}
