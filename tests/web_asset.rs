#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(all(target_arch = "wasm32"))]
use bitcoin::{consensus, Transaction};
use bitmask_core::{
    debug, info,
    structs::{
        EncryptedWalletData, FundVaultDetails, ImportRequest, ImportType, MnemonicSeedData,
        WalletData,
    },
    web::{
        bitcoin::{get_assets_vault, get_encrypted_wallet, get_wallet_data, save_mnemonic_seed},
        json_parse, resolve,
        rgb::import_contract,
        set_panic_hook,
    },
};
use futures::executor::block_on;
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[wasm_bindgen_test]
async fn test_blocking_client() -> anyhow::Result<()> {
    let txid = bitcoin::Txid::from_str(
        "6a64b7ed232f6d66409ad6716f51b5915ca999b3da356d924aae48dc7fcd3e04",
    )?;

    block_on(async {
        let final_url = &format!("{}/tx/{}/raw", "https://mempool.space/testnet/api", txid);
        let result = surf::get(final_url).recv_bytes().await.expect("");
        let tx: Transaction = consensus::deserialize::<Transaction>(&result).expect("");
        println!("{:?}", tx);
    });

    Ok(())
}

#[wasm_bindgen_test]
async fn contract_import() {
    set_panic_hook();
    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");

    info!("Import wallet");
    let mnemonic_data_str = resolve(save_mnemonic_seed(
        mnemonic.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    let mnemonic_data: MnemonicSeedData = json_parse(&mnemonic_data_str);

    info!("Get vault properties");
    let vault_str: JsValue = resolve(get_encrypted_wallet(
        ENCRYPTION_PASSWORD.to_owned(),
        mnemonic_data.serialized_encrypted_message,
    ))
    .await;
    let wallet_data: EncryptedWalletData = json_parse(&vault_str);

    info!("Import Contract");
    let sk = wallet_data.private.nostr_prv;
    let contract_import = ImportRequest {
        import: ImportType::Contract,
        data: "".to_string(),
    };

    let req = serde_wasm_bindgen::to_value(&contract_import).expect("");
    let _ = resolve(import_contract(sk, req)).await;
}

#[wasm_bindgen_test]
async fn asset_transfer() {
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
}
