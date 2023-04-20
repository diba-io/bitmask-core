#![cfg(all(target_arch = "wasm32"))]
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use bitmask_core::{
    debug, info,
    web::{
        get_assets_vault, get_encrypted_wallet, get_wallet_data, json_parse, resolve,
        save_mnemonic_seed, set_panic_hook,
    },
    EncryptedWalletData, FundVaultDetails, MnemonicSeedData, WalletData,
};

wasm_bindgen_test_configure!(run_in_browser);

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

/// Test asset import
#[wasm_bindgen_test]
async fn asset_import() {
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
    let wallet_data_str: JsValue = resolve(get_encrypted_wallet(
        ENCRYPTION_PASSWORD.to_owned(),
        mnemonic_data.serialized_encrypted_message,
    ))
    .await;
    let wallet_data: EncryptedWalletData = json_parse(&wallet_data_str);

    info!("Get Wallets");
    let assets_wallet = resolve(get_wallet_data(
        wallet_data.public.rgb_assets_descriptor_xpub.clone(),
        None,
    ))
    .await;
    let assets_wallet: WalletData = json_parse(&assets_wallet);
    let udas_wallet = resolve(get_wallet_data(
        wallet_data.public.rgb_udas_descriptor_xpub.clone(),
        None,
    ))
    .await;
    let udas_wallet: WalletData = json_parse(&udas_wallet);

    info!("Check Asset Vault");
    let vault_details = resolve(get_assets_vault(
        wallet_data.public.rgb_assets_descriptor_xpub.clone(),
        wallet_data.public.rgb_udas_descriptor_xpub,
    ))
    .await;
    let vault_details: FundVaultDetails = json_parse(&vault_details);

    // TODO: WASM asset test

    // info!("Check Main Asset Vault");
    // if vault_details.assets_output.is_none() {
    //     info!("Missing an asset UTXO in vault. Funding vault...");
    //     let new_vault_details = resolve(fund_vault(
    //         wallet_data.btc_descriptor_xprv,
    //         wallet_data.btc_change_descriptor_xprv,
    //         assets_wallet.address,
    //         udas_wallet.address,
    //         1546,
    //         1546,
    //         Some(1.0),
    //     ))
    //     .await;
    //     vault_details = json_parse(&new_vault_details);
    //     debug!("Fund vault details: {assets_vault_details:#?}");
    // }

    // resolve(import_asset(
    //     ASSET.to_owned(),
    //     wallet_data.rgb_assets_descriptor_xpub,
    // ))
    // .await;

    // // Get wallet data
    // let wallet_str: JsValue = resolve(get_wallet_data(
    //     wallet_data.rgb_assets_descriptor_xprv.clone(),
    //     None,
    // ))
    // .await;

    // // Parse wallet data
    // let wallet_data: WalletData = json_parse(&wallet_str);

    // assert_eq!(wallet_data.transactions, vec![]);
}
