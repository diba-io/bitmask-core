#![cfg(not(target_arch = "wasm32"))]

use std::env;

use anyhow::Result;
use bitmask_core::{
    create_asset, fund_vault, get_assets_vault, get_blinded_utxo, get_mnemonic_seed, get_network,
    get_vault, get_wallet_data, import_asset, save_mnemonic_seed, send_assets, send_sats,
};
use log::{debug, info};

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const TICKER: &str = "TEST";
const NAME: &str = "Test asset";
const PRECISION: u8 = 3;
const SUPPLY: u64 = 1000;

/// Test asset import
#[tokio::test]
async fn asset_transfer() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var(
            "RUST_LOG",
            "bitmask_core=debug,bitmask_core::operations::rgb::send_tokens=trace,asset=debug",
        );
    }

    pretty_env_logger::init();

    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");
    let main_mnemonic_data = save_mnemonic_seed(main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_vault(
        ENCRYPTION_PASSWORD,
        &main_mnemonic_data.serialized_encrypted_message,
    )?;

    let main_btc_wallet = get_wallet_data(
        &main_vault.btc_descriptor_xprv,
        Some(main_vault.btc_change_descriptor_xprv.clone()),
    )
    .await?;

    info!("Main vault address: {}", main_btc_wallet.address);

    assert!(
        !main_btc_wallet.transactions.is_empty(),
        "Main wallet transactions list is empty (has this wallet been funded?)"
    );

    info!("Create an ephemeral wallet from fresh mnemonic for purposes of the test");
    let tmp_mnemonic = get_mnemonic_seed(ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    info!("Get ephemeral vault properties");
    let tmp_vault = get_vault(
        ENCRYPTION_PASSWORD,
        &tmp_mnemonic.serialized_encrypted_message,
    )?;

    info!("Get sats wallet data");
    let btc_wallet = get_wallet_data(
        &tmp_vault.btc_descriptor_xprv,
        Some(tmp_vault.btc_change_descriptor_xprv.clone()),
    )
    .await?;

    info!("Fund ephemeral wallet");
    let tx = send_sats(
        &main_vault.btc_descriptor_xprv,
        &main_vault.btc_change_descriptor_xprv,
        &btc_wallet.address,
        5000,
        Some(1.0),
    )
    .await?;

    debug!("Ephemeral wallet funding tx: {tx:?}");

    info!("Get assets wallet data");
    let assets_wallet = get_wallet_data(&tmp_vault.rgb_assets_descriptor_xpub, None).await?;

    info!("Get UDAs wallet data");
    let udas_wallet = get_wallet_data(&tmp_vault.rgb_udas_descriptor_xpub, None).await?;

    info!("Check assets vault");
    let fund_vault_details = get_assets_vault(
        &tmp_vault.rgb_assets_descriptor_xpub,
        &tmp_vault.rgb_udas_descriptor_xpub,
    )
    .await?;

    let send_assets_utxo = match fund_vault_details.assets_change_output {
        Some(send_assets_utxo) => send_assets_utxo,
        None => {
            info!("Missing an asset UTXO in vault. Funding vault...");
            let fund_vault_details = fund_vault(
                &tmp_vault.btc_descriptor_xprv,
                &tmp_vault.btc_change_descriptor_xprv,
                &assets_wallet.address,
                &udas_wallet.address,
                546,
                546,
                Some(3.0),
            )
            .await?;
            debug!("Fund vault details: {fund_vault_details:#?}");
            fund_vault_details.assets_output.unwrap()
        }
    };

    info!("Create a test asset");
    let issued_asset = &create_asset(TICKER, NAME, PRECISION, SUPPLY, &send_assets_utxo)?;

    let asset_data = serde_json::to_string_pretty(&issued_asset)?;
    debug!("Asset data: {asset_data}");

    info!("Import asset");
    let imported_asset = import_asset(&issued_asset.genesis)?;

    assert_eq!(issued_asset.asset_id, imported_asset.id, "Asset IDs match");

    info!("Get a blinded UTXO");
    let blinded_utxo = get_blinded_utxo(&send_assets_utxo)?;

    debug!("Blinded UTXO: {:?}", blinded_utxo);

    info!("Transfer asset");
    let consignment_details = send_assets(
        &tmp_vault.btc_descriptor_xprv,
        &tmp_vault.btc_change_descriptor_xpub,
        &tmp_vault.rgb_assets_descriptor_xprv,
        &tmp_vault.rgb_assets_descriptor_xpub,
        &blinded_utxo.conceal,
        100,
        &issued_asset.genesis,
        3.0,
    )
    .await?;

    debug!("Transfer response: {:#?}", &consignment_details);

    Ok(())
}
