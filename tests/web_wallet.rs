#![cfg(all(target_arch = "wasm32"))]

use std::env;

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use bdk::TransactionDetails;

use bitmask_core::{
    web::{
        get_mnemonic_seed, get_vault, get_wallet_data, json_parse, resolve, save_mnemonic_seed,
        send_sats, set_panic_hook, to_string,
    },
    MnemonicSeedData, VaultData, WalletData,
};

wasm_bindgen_test_configure!(run_in_browser);

const MNEMONIC: &str =
    "swing rose forest coral approve giggle public liar brave piano sound spirit";
const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const DESCRIPTOR: &str = "wpkh([a4a469b0/84'/1'/0'/0]tprv8haHNLCCjhGAdYZinundP58hLrv6325kQGpv2mdE2wfb9WkvqXGVj5fFuqfpJSDS1AQCBGLvjrLszHPHUqewVQeYCiecySr4FSHqzStedLM/*)";
const CHANGE_DESCRIPTOR: &str = "wpkh([a4a469b0/84'/1'/0'/1]tprv8haHNLCCjhGAiUUDixqL9yMdZMTi8a9pNdLSBg92QkZUHCKEc4Uo2Rg4uZPHGtDheJvpLvwLm8hXErKbCXe96kD453jHYtBJkmLNGNYV9Yx/*)";
const PUBKEY_HASH: &str = "a4a469b0a03e479500ad438b44a45c8ba3246482";
const ADDRESS: &str = "tb1qh89unmzv905qpm8c3u84wa42jr290mjkxyc5an";

/// Tests for Wallet Creation Workflow

/// Create wallet
#[wasm_bindgen_test]
async fn create_wallet() {
    set_panic_hook();

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
    set_panic_hook();

    // Import wallet
    let mnemonic_data_str = resolve(save_mnemonic_seed(
        MNEMONIC.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;

    let mnemonic_data: MnemonicSeedData = json_parse(&mnemonic_data_str);
    let encrypted_descriptors =
        serde_json::to_string(&mnemonic_data.serialized_encrypted_message).unwrap();

    // Get vault properties
    let vault_str: JsValue = resolve(get_vault(
        ENCRYPTION_PASSWORD.to_owned(),
        encrypted_descriptors,
    ))
    .await;
    let vault_data: VaultData = json_parse(&vault_str);

    assert_eq!(vault_data.btc_descriptor_xprv, DESCRIPTOR);
    assert_eq!(vault_data.btc_change_descriptor_xprv, CHANGE_DESCRIPTOR);
    assert_eq!(vault_data.xpubkh, PUBKEY_HASH);

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        DESCRIPTOR.to_owned(),
        Some(CHANGE_DESCRIPTOR.to_owned()),
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = json_parse(&wallet_str);

    assert_eq!(
        wallet_data.address,
        ADDRESS.to_owned(),
        "parsed wallet data matches address"
    );
    assert_eq!(wallet_data.balance, "0");
    assert_eq!(wallet_data.transactions, vec![]);

    // Set blinded UTXOs
    // todo!("Same but with blinded_utxo?");
    // resolve(set_blinded_utxos("[]".to_owned(), "{}".to_owned())).await;
}

/// Can import the testing mnemonic
/// Can open a wallet and view address and balance
#[wasm_bindgen_test]
async fn import_test_wallet() {
    set_panic_hook();

    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");

    // Import wallet
    let mnemonic_data_str = resolve(save_mnemonic_seed(
        mnemonic.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;

    let mnemonic_data: MnemonicSeedData = json_parse(&mnemonic_data_str);
    let encrypted_descriptors =
        serde_json::to_string(&mnemonic_data.serialized_encrypted_message).unwrap();

    // Get vault properties
    let vault_str: JsValue = resolve(get_vault(
        ENCRYPTION_PASSWORD.to_owned(),
        encrypted_descriptors,
    ))
    .await;
    let vault_data: VaultData = json_parse(&vault_str);

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        vault_data.btc_descriptor_xprv.clone(),
        Some(vault_data.btc_change_descriptor_xprv.clone()),
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = json_parse(&wallet_str);

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
            .confirmation_time
            .is_some(),
        "last transaction is confirmed"
    );
    assert!(
        wallet_data
            .transactions
            .last()
            .expect("transactions already in wallet")
            .confirmed,
        "last transaction has the confirmed property and is true"
    );

    // Test sending a transaction back to itself for a thousand sats
    let tx_details = resolve(send_sats(
        vault_data.btc_descriptor_xprv,
        vault_data.btc_change_descriptor_xprv,
        wallet_data.address,
        1_000,
        None,
    ))
    .await;

    // Parse tx_details
    let tx_data: TransactionDetails = json_parse(&tx_details);

    assert!(
        tx_data.confirmation_time.is_none(),
        "latest transaction hasn't been confirmed yet"
    );
}
