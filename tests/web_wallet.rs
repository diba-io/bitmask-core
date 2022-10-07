#![cfg(all(target_arch = "wasm32"))]

use std::env;

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use bdk::TransactionDetails;

use bitmask_core::{
    web::{
        get_encrypted_wallet, get_mnemonic_seed, get_wallet_data, json_parse, resolve,
        save_mnemonic_seed, send_sats, set_panic_hook, to_string,
    },
    EncryptedWalletData, MnemonicSeedData, WalletData,
};

wasm_bindgen_test_configure!(run_in_browser);

const MNEMONIC: &str =
    "swing rose forest coral approve giggle public liar brave piano sound spirit";
const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const DESCRIPTOR: &str = "tr([a4a469b0/86'/1'/0']tprv8gM7m1SkWiAK7v1B1cxP5843bd7fqbEr4JU3r9P2ZiN5C8uiM2nuxStGctYXLY6zVKYM4qRd37QDn1iiMkt4pqopgWCht5eq6d3ZFPYh2VS/0/*)";
const CHANGE_DESCRIPTOR: &str = "tr([a4a469b0/86'/1'/0']tprv8gM7m1SkWiAK7v1B1cxP5843bd7fqbEr4JU3r9P2ZiN5C8uiM2nuxStGctYXLY6zVKYM4qRd37QDn1iiMkt4pqopgWCht5eq6d3ZFPYh2VS/1/*)";
const PUBKEY_HASH: &str = "a4a469b0a03e479500ad438b44a45c8ba3246482";
const ADDRESS: &str = "tb1pjy3xavhaut6qjkkggh5k87qj7d9am8d02ug8astan54538dcthkqqg6zf5";

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

    // Get encrypted wallet properties
    let encrypted_wallet_str: JsValue = resolve(get_encrypted_wallet(
        ENCRYPTION_PASSWORD.to_owned(),
        mnemonic_data.serialized_encrypted_message,
    ))
    .await;
    let encrypted_wallet_data: EncryptedWalletData = json_parse(&encrypted_wallet_str);

    assert_eq!(encrypted_wallet_data.btc_descriptor_xprv, DESCRIPTOR);
    assert_eq!(
        encrypted_wallet_data.btc_change_descriptor_xprv,
        CHANGE_DESCRIPTOR
    );
    assert_eq!(encrypted_wallet_data.xpubkh, PUBKEY_HASH);

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        DESCRIPTOR.to_owned(),
        Some(CHANGE_DESCRIPTOR.to_owned()),
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = json_parse(&wallet_str);

    assert_eq!(
        &wallet_data.address, ADDRESS,
        "parsed wallet data matches address"
    );
    assert!(wallet_data.balance.confirmed > 0);
    assert!(!wallet_data.transactions.is_empty());
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

    // Get vault properties
    let vault_str: JsValue = resolve(get_encrypted_wallet(
        ENCRYPTION_PASSWORD.to_owned(),
        mnemonic_data.serialized_encrypted_message,
    ))
    .await;
    let encrypted_wallet_data: EncryptedWalletData = json_parse(&vault_str);

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        encrypted_wallet_data.btc_descriptor_xprv.clone(),
        Some(encrypted_wallet_data.btc_change_descriptor_xprv.clone()),
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = json_parse(&wallet_str);

    assert!(
        wallet_data.balance.confirmed > 0,
        "test wallet balance is greater than zero"
    );
    assert!(
        wallet_data
            .transactions
            .first()
            .expect("transactions already in wallet")
            .confirmation_time
            .is_some(),
        "first transaction is confirmed"
    );
    assert!(
        wallet_data
            .transactions
            .first()
            .expect("transactions already in wallet")
            .confirmed,
        "first transaction has the confirmed property and is true"
    );

    // Test sending a transaction back to itself for a thousand sats
    let tx_details = resolve(send_sats(
        encrypted_wallet_data.btc_descriptor_xprv,
        encrypted_wallet_data.btc_change_descriptor_xprv,
        wallet_data.address,
        1_000,
        Some(1.0),
    ))
    .await;

    // Parse tx_details
    let tx_data: TransactionDetails = json_parse(&tx_details);

    assert!(
        tx_data.confirmation_time.is_none(),
        "latest transaction hasn't been confirmed yet"
    );
}
