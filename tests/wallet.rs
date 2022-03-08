#![cfg(target_arch = "wasm32")]

use std::env;

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use bitmask_core::{
    get_mnemonic_seed, get_vault, get_wallet_data, resolve, save_mnemonic_seed, set_blinded_utxos,
    to_string, VaultData, WalletData,
};

wasm_bindgen_test_configure!(run_in_browser);

const MNEMONIC: &str = "then kidney town pair iron agent assault put oven erosion like govern";
const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";
const DESCRIPTOR: &str = "wpkh([0c45fbf7/84'/1'/0'/0]tprv8hk1wQ9P3PCqjxN9WwcDmDni8FPcXD5wFbPiDGVVutQMaXjwm4iMRWyuvVXpWWn61M2DX3a1JquTXEGmVYi4P7Ep2zvtt2JAcSSaYkgZYHG/*)";
const CHANGE_DESCRIPTOR: &str = "wpkh([0c45fbf7/84'/1'/0'/1]tprv8hk1wQ9P3PCqopBdG2rcVfWCZ2cVmF759KAVk6eFj68v52vQVbNT5PiN4bVwgtyUQzYWs3kM9m7Pe6HmoeVbEnPrww2smcVkqe3qFLJt3wx/*)";
const PUBKEY_HASH: &str = "0c45fbf798037b051ac501ac3f56e8b4656f930a";
const ADDRESS: &str = "tb1q6phj46ulkrxzht5se7huxc2gk7t8dsl6uasg36";

/// Tests for Wallet Creation Workflow

/// Create wallet
#[wasm_bindgen_test]
async fn create_wallet() {
    // Mnemonic string is 12 words long
    let mnemonic: JsValue = resolve(get_mnemonic_seed(
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    assert!(!mnemonic.is_undefined());
    assert!(mnemonic.is_string());
    assert_eq!(to_string(&mnemonic).split(' ').count(), 12);
}

/// Can import a hardcoded mnemonic
/// Can open a wallet and view address and balance
#[wasm_bindgen_test]
async fn import_and_open_wallet() {
    // Import wallet
    resolve(save_mnemonic_seed(
        MNEMONIC.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;

    // Get vault properties
    let vault_str: JsValue = resolve(get_vault(ENCRYPTION_PASSWORD.to_owned())).await;
    let vault_data: VaultData = serde_json::from_str(&to_string(&vault_str)).unwrap();

    assert_eq!(vault_data.descriptor, DESCRIPTOR);
    assert_eq!(vault_data.change_descriptor, CHANGE_DESCRIPTOR);
    assert_eq!(vault_data.pubkey_hash, PUBKEY_HASH);

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        DESCRIPTOR.to_owned(),
        CHANGE_DESCRIPTOR.to_owned(),
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = serde_json::from_str(&to_string(&wallet_str)).unwrap();

    assert_eq!(
        wallet_data.address,
        ADDRESS.to_owned(),
        "parsed wallet data matches address"
    );
    assert_eq!(wallet_data.balance, "0");
    assert_eq!(wallet_data.transactions, vec![]);

    // Set blinded UTXOs
    resolve(set_blinded_utxos()).await;
}

/// Can import the testing mnemonic
/// Can open a wallet and view address and balance
#[wasm_bindgen_test]
async fn import_test_wallet() {
    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");

    // Import wallet
    resolve(save_mnemonic_seed(
        mnemonic.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;

    // Get vault properties
    let vault_str: JsValue = resolve(get_vault(ENCRYPTION_PASSWORD.to_owned())).await;
    let vault_data: VaultData = serde_json::from_str(&to_string(&vault_str)).unwrap();

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        vault_data.descriptor,
        vault_data.change_descriptor,
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = serde_json::from_str(&to_string(&wallet_str)).unwrap();

    assert!(
        wallet_data
            .balance
            .parse::<f64>()
            .expect("parsed wallet balance")
            > 0.0,
        "test wallet balance is greater than zero"
    );
    assert!(
        wallet_data
            .transactions
            .last()
            .expect("transactions already in wallet")
            .confirmed,
        "last transaction is confirmed"
    );
}
