#![cfg(not(target_arch = "wasm32"))]

use std::env;

use anyhow::Result;
use bitmask_core::{
    /* create_asset, */ fund_wallet, get_network, get_vault,
    get_wallet_data, /* import_asset, */
    save_mnemonic_seed, set_blinded_utxo,
};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

// const TICKER: &str = "TEST";
// const NAME: &str = "Test asset";
// const PRECISION: u8 = 3;
// const SUPPLY: u64 = 1000;

/// Test asset import
#[tokio::test]
async fn asset_import() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    pretty_env_logger::init();

    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallet");
    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");
    let mnemonic_data = save_mnemonic_seed(mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let encrypted_descriptors = serde_json::to_string(&mnemonic_data.serialized_encrypted_message)?;

    info!("Get vault properties");
    let vault = get_vault(ENCRYPTION_PASSWORD, &encrypted_descriptors)?;

    info!("Get assets wallet data");
    let btc_wallet =
        get_wallet_data(&vault.btc_descriptor, Some(&vault.btc_change_descriptor)).await?;

    info!("Get assets wallet data");
    let assets_wallet = get_wallet_data(&vault.rgb_tokens_descriptor, None).await?;

    info!("Get UDAs wa&llet data");
    let udas_wallet = get_wallet_data(&vault.rgb_nfts_descriptor, None).await?;

    info!("Fund vault");
    let fund_vault_details = fund_wallet(
        &vault.btc_descriptor,
        &vault.btc_change_descriptor,
        &assets_wallet.address,
        &udas_wallet.address,
    )
    .await?;

    // info!("Create a test asset");
    // let (genesis, _) = create_asset(
    //     TICKER,
    //     NAME,
    //     PRECISION,
    //     SUPPLY,
    //     &fund_vault_details.send_assets,
    // )?;

    // let asset_id = genesis.contract_id().to_string();

    // let asset = import_asset(&vault.rgb_tokens_descriptor, None, genesis, None).await?;

    // assert_eq!(asset.id, asset_id, "Asset IDs match");

    info!("Parse wallet data");
    assert!(
        !btc_wallet.transactions.is_empty(),
        "list of transactions is empty"
    );

    set_blinded_utxo(&fund_vault_details.send_assets)?;

    Ok(())
}
