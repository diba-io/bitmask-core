#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use bitmask_core::{
    get_vault, get_wallet_data, import_asset, json_parse, resolve, save_mnemonic_seed,
    set_panic_hook, MnemonicSeedData, VaultData, WalletData,
};

wasm_bindgen_test_configure!(run_in_browser);

const MNEMONIC: &str =
    "swing rose forest coral approve giggle public liar brave piano sound spirit";
const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const ASSET: &str = "rgb1g2antx89ypjuat7jdth35d8xgqserckrhj9elkrhxhjhxch8sxqqguzmh6"; // STC, StableCoin
const GENESIS: &str = "genesis1qyfe883hey6jrgj2xvk5g3dfmfqfzm7a4wez4pd2krf7ltsxffd6u6nrvjvvnc8vt9llmp7663pgututl9heuwaudet72ay9j6thc6cetuvhxvsqqya5xjt2w9y4u6sfkuszwwctnrpug5yjxnthmr3mydg05rdrpspcxysnqvvqpfvag2w8jxzzsz9pf8pjfwf0xvln5z7w93yjln3gcnyxsa04jsf2p8vu4sxgeqrzyxg5nyvcysuur9qjct5xuzfvffyu23n6p22vaqpvcryvvqrnqvnswv7r3xqgxf3qryxransvgre33asjzqerx0vpe9lff02guztd6xyd5hwq0w37e0cqmutvm428mnmaayhlhfj4nh0zaalutdrurrnlets8axpxkfcqgpmrxqqqxsu7qc";

/// Test asset import
#[wasm_bindgen_test]
async fn asset_import() {
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

    resolve(import_asset(
        vault_data.rgb_tokens_descriptor.clone(),
        Some(ASSET.to_owned()),
        Some(GENESIS.to_owned()),
    ))
    .await;

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        vault_data.rgb_tokens_descriptor.clone(),
        None,
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = json_parse(&wallet_str);

    assert_eq!(wallet_data.transactions, vec![]);
}
