#![cfg(all(target_arch = "wasm32"))]
use bitmask_core::{
    debug, info,
    structs::{EncryptedWalletData, MnemonicSeedData, TransactionDetails, WalletData},
    web::{
        bitcoin::{
            get_encrypted_wallet, get_mnemonic_seed, get_wallet_data, hash_password,
            save_mnemonic_seed, send_sats,
        },
        json_parse, resolve, set_panic_hook, to_string,
    },
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const MNEMONIC: &str =
    "outdoor nation key manual street net kidney insect ranch dial follow furnace";
const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const DESCRIPTOR: &str = "tr([41e7fa8b/86'/1'/0']tprv8fddZuQpcukmaC4nND6cQTbPsim88ArLnVT6K2Vcnzi37gDFh7EhtKaEdDqyUGc1mRwVyPzkbNe2ZWd8Ryj5CWMRpLDn3ppKtgozUvp17rv/0/*)";
const CHANGE_DESCRIPTOR: &str = "tr([41e7fa8b/86'/1'/0']tprv8fddZuQpcukmaC4nND6cQTbPsim88ArLnVT6K2Vcnzi37gDFh7EhtKaEdDqyUGc1mRwVyPzkbNe2ZWd8Ryj5CWMRpLDn3ppKtgozUvp17rv/1/*)";
const PUBKEY_HASH: &str = "41e7fa8bc772add75092e31f0a15c10675163e82";

/// Tests for Wallet Creation Workflow

/// Create wallet
#[wasm_bindgen_test]
async fn create_wallet() {
    set_panic_hook();

    info!("Mnemonic string is 12 words long");
    let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());
    let mnemonic: JsValue = resolve(get_mnemonic_seed(hash, SEED_PASSWORD.to_owned())).await;
    assert!(!mnemonic.is_undefined());
    assert!(mnemonic.is_string());
    assert_eq!(to_string(&mnemonic).split(' ').count(), 12);
}

/// Can import a hardcoded mnemonic
/// Can open a wallet and view address and balance
#[wasm_bindgen_test]
async fn import_and_open_wallet() {
    set_panic_hook();

    info!("Import wallet");
    let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());
    let mnemonic_data_str = resolve(save_mnemonic_seed(
        MNEMONIC.to_owned(),
        hash.clone(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;

    let mnemonic_data: MnemonicSeedData = json_parse(&mnemonic_data_str);

    info!("Get encrypted wallet properties");
    let encrypted_wallet_str: JsValue = resolve(get_encrypted_wallet(
        hash,
        mnemonic_data.serialized_encrypted_message,
    ))
    .await;
    let encrypted_wallet_data: EncryptedWalletData = json_parse(&encrypted_wallet_str);

    assert_eq!(
        encrypted_wallet_data.private.btc_descriptor_xprv, DESCRIPTOR,
        "expected receive descriptor matches loaded wallet"
    );
    assert_eq!(
        encrypted_wallet_data.private.btc_change_descriptor_xprv, CHANGE_DESCRIPTOR,
        "expected change descriptor matches loaded wallet"
    );
    assert_eq!(
        encrypted_wallet_data.public.xpubkh, PUBKEY_HASH,
        "expected xpubkh matches loaded wallet"
    );

    info!("Get wallet data");
    let wallet_str: JsValue = resolve(get_wallet_data(
        DESCRIPTOR.to_owned(),
        Some(CHANGE_DESCRIPTOR.to_owned()),
    ))
    .await;

    info!("Parse wallet data");
    let wallet_data: WalletData = json_parse(&wallet_str);

    assert_eq!(wallet_data.balance.confirmed, 0, "wallet has no sats");
    assert!(wallet_data.transactions.is_empty(), "wallet has no txs");
}

/// Can import the testing mnemonic
/// Can open a wallet and view address and balance
#[wasm_bindgen_test]
async fn import_test_wallet() {
    set_panic_hook();

    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");

    info!("Import wallet");
    let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());
    let mnemonic_data_str = resolve(save_mnemonic_seed(
        mnemonic.to_owned(),
        hash.clone(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    let mnemonic_data: MnemonicSeedData = json_parse(&mnemonic_data_str);

    info!("Get vault properties");
    let vault_str: JsValue = resolve(get_encrypted_wallet(
        hash,
        mnemonic_data.serialized_encrypted_message,
    ))
    .await;
    let encrypted_wallet_data: EncryptedWalletData = json_parse(&vault_str);

    info!("Get wallet data");
    let wallet_str: JsValue = resolve(get_wallet_data(
        encrypted_wallet_data.private.btc_descriptor_xprv.clone(),
        Some(
            encrypted_wallet_data
                .private
                .btc_change_descriptor_xprv
                .clone(),
        ),
    ))
    .await;
    let wallet_data: WalletData = json_parse(&wallet_str);

    debug!(format!("Wallet address: {}", wallet_data.address));

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

    info!("Test sending a transaction back to itself for a thousand sats");
    let tx_details = resolve(send_sats(
        encrypted_wallet_data.private.btc_descriptor_xprv,
        encrypted_wallet_data.private.btc_change_descriptor_xprv,
        wallet_data.address,
        1_000,
        Some(1.1),
    ))
    .await;

    info!("Parse tx_details");
    let tx_data: TransactionDetails = json_parse(&tx_details);

    assert!(
        tx_data.confirmation_time.is_none(),
        "latest transaction hasn't been confirmed yet"
    );
}
