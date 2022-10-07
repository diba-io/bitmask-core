// #![cfg(all(target_arch = "wasm32"))]

// use wasm_bindgen::prelude::*;
// use wasm_bindgen_test::*;

// use bitmask_core::{
//     web::{
//         get_encrypted_wallet, get_wallet_data, import_asset, json_parse, resolve,
//         save_mnemonic_seed, set_panic_hook,
//     },
//     EncryptedWalletData, MnemonicSeedData, WalletData,
// };

// wasm_bindgen_test_configure!(run_in_browser);

// const MNEMONIC: &str =
//     "swing rose forest coral approve giggle public liar brave piano sound spirit";
// const ENCRYPTION_PASSWORD: &str = "hunter2";
// const SEED_PASSWORD: &str = "";

// const ASSET: &str = "rgb1g2antx89ypjuat7jdth35d8xgqserckrhj9elkrhxhjhxch8sxqqguzmh6"; // BUX

// /// Test asset import
// #[wasm_bindgen_test]
// async fn asset_import() {
//     set_panic_hook();

//     // Import wallet
//     let mnemonic_data_str = resolve(save_mnemonic_seed(
//         MNEMONIC.to_owned(),
//         ENCRYPTION_PASSWORD.to_owned(),
//         SEED_PASSWORD.to_owned(),
//     ))
//     .await;

//     let mnemonic_data: MnemonicSeedData = json_parse(&mnemonic_data_str);
//     let encrypted_descriptors =
//         serde_json::to_string(&mnemonic_data.serialized_encrypted_message).unwrap();

//     // Get vault properties
//     let wallet_data_str: JsValue = resolve(get_encrypted_wallet(
//         ENCRYPTION_PASSWORD.to_owned(),
//         encrypted_descriptors,
//     ))
//     .await;
//     let encrypted_wallet_data: EncryptedWalletData = json_parse(&wallet_data_str);

//     resolve(import_asset(
//         ASSET.to_owned(),
//         encrypted_wallet_data.rgb_assets_descriptor_xpub.clone(),
//     ))
//     .await;

//     // Get wallet data
//     let wallet_str: JsValue = resolve(get_wallet_data(
//         encrypted_wallet_data.rgb_assets_descriptor_xprv.clone(),
//         None,
//     ))
//     .await;

//     // Parse wallet data
//     let wallet_data: WalletData = json_parse(&wallet_str);

//     assert_eq!(wallet_data.transactions, vec![]);
// }
