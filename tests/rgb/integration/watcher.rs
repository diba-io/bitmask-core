#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::{
    bitcoin::{get_wallet, save_mnemonic, sync_wallet},
    constants::switch_network,
    rgb::{
        create_watcher, get_contract, watcher_address, watcher_next_address, watcher_next_utxo,
        watcher_utxo,
    },
    structs::{SecretString, WatcherRequest},
};

use crate::rgb::integration::utils::{send_some_coins, ISSUER_MNEMONIC, OWNER_MNEMONIC};

#[tokio::test]
async fn allow_monitoring_address() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Create Watcher
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };

    create_watcher(&sk, create_watch_req.clone()).await?;

    // Get Address
    let issuer_wallet = get_wallet(
        &SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
        None,
    )
    .await?;
    sync_wallet(&issuer_wallet).await?;

    let address = issuer_wallet
        .lock()
        .await
        .get_address(bdk::wallet::AddressIndex::LastUnused)?;

    // Register Address (Watcher)
    let resp = watcher_address(&sk, watcher_name, &address.address.to_string()).await;
    assert!(resp.is_ok());
    assert!(resp?.utxos.is_empty());
    Ok(())
}

#[tokio::test]
async fn allow_monitoring_address_with_coins() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Create Watcher
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };

    create_watcher(&sk, create_watch_req.clone()).await?;

    // Get Address
    let issuer_wallet = get_wallet(
        &SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
        None,
    )
    .await?;
    sync_wallet(&issuer_wallet).await?;

    let address = issuer_wallet
        .lock()
        .await
        .get_address(bdk::wallet::AddressIndex::LastUnused)?;
    let address = address.address.to_string();

    // Send some coins
    send_some_coins(&address, "0.01").await;

    // Register Address (Watcher)
    let resp = watcher_address(&sk, watcher_name, &address).await;
    assert!(resp.is_ok());
    assert!(!resp?.utxos.is_empty());
    Ok(())
}

#[tokio::test]
async fn allow_monitoring_invalid_utxo() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Create Watcher
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&sk, create_watch_req.clone()).await?;

    // Get UTXO
    let next_utxo = "a6bbd6839ed4ad9ce53cf8bb56a01792031bfee6eed20877311408f2187bc239:0";

    // Force Watcher (Recreate)
    create_watcher(&sk, create_watch_req.clone()).await?;

    // Register Utxo (Watcher)
    let resp = watcher_utxo(&sk, watcher_name, next_utxo).await;
    assert!(resp.is_ok());
    assert!(resp?.utxos.is_empty());
    Ok(())
}

#[tokio::test]
async fn allow_monitoring_valid_utxo() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Create Watcher
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&sk, create_watch_req.clone()).await?;

    // Get Address
    let next_addr = watcher_next_address(&sk, watcher_name, "RGB20").await?;

    // Send some coins
    send_some_coins(&next_addr.address, "0.01").await;

    // Get UTXO
    let next_utxo = watcher_next_utxo(&sk, watcher_name, "RGB20").await?;

    // Force Watcher (Recreate)
    create_watcher(&sk, create_watch_req.clone()).await?;

    // Register Utxo (Watcher)
    let resp = watcher_utxo(&sk, watcher_name, &next_utxo.utxo.unwrap().outpoint).await;
    assert!(resp.is_ok());
    assert!(!resp?.utxos.is_empty());
    Ok(())
}

#[tokio::test]
async fn allow_migrate_watcher() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    let owner_keys = &save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Create Watcher (Wrong Key)
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: false,
    };

    create_watcher(&sk, create_watch_req.clone()).await?;

    // Create Watcher (Correct Key)
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: false,
    };

    let resp = create_watcher(&sk, create_watch_req.clone()).await?;
    assert!(resp.migrate);
    Ok(())
}

#[ignore]
#[tokio::test]
async fn reproduce_jose_transfer_tests() -> anyhow::Result<()> {
    switch_network("testnet").await?;
    // 1. Initial Setup
    let _scontract_id = "rgb:24bsMkisNQEhWT4fB6WgrG3kvs8XnW1kwQa2m852sGVEEscPha";
    let wallet_a_keys = save_mnemonic(
        &SecretString("priority palace course actor exercise silver donkey prize blast tool discover hunt cup vast dash universe slam onion wall indoor correct mechanic pink wink".to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    let wallet_b_keys = save_mnemonic(
        &SecretString("whisper cloud movie wood soon stumble journey assist town wrong unique love reward produce faith wine sponsor label fine upon cargo plate cash owner".to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    let contract_id = "rgb:mbEPRYUwcqkqsG9KMVY88ANkiHuEe5j4x67LjsPgBhD22Nzcj";

    let wallet_a_sk = &wallet_a_keys.private.nostr_prv;
    let contract = get_contract(wallet_a_sk, contract_id).await?;
    // for contract in contracts.contracts {
    println!(
        "Wallet A: {} ({})\nAllocs: {:#?}",
        contract.contract_id, contract.ticker, contract.allocations
    );
    // }

    let wallet_b_sk = &wallet_b_keys.private.nostr_prv;
    let contract = get_contract(wallet_b_sk, contract_id).await?;
    // for contract in contracts.contracts {
    println!(
        "Wallet B: {} ({})\nAllocs: {:#?}",
        contract.contract_id, contract.ticker, contract.allocations
    );
    // }

    Ok(())
}
