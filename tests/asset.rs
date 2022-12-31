#![cfg(not(target_arch = "wasm32"))]

use core::time;
use std::{collections::BTreeMap, env, str::FromStr, thread};

use anyhow::Result;
use bitcoin::OutPoint;
use bitmask_core::{
    accept_transfer, create_asset,
    data::structs::{AssetTransfer, AssetUtxo},
    fund_vault, get_assets_vault, get_blinded_utxo, get_encrypted_wallet, get_mnemonic_seed,
    get_network, get_wallet_data, import_asset, save_mnemonic_seed, send_sats, transfer_assets,
    TransfersRequest,
};
use log::{debug, info};

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const TICKER: &str = "TEST";
const NAME: &str = "Test asset";
const PRECISION: u8 = 3;
const SUPPLY: u64 = 1000;

#[tokio::test]
async fn allow_transfer_one_asset_to_one_beneficiary() -> Result<()> {
    let _ = pretty_env_logger::try_init();
    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let main_mnemonic_data =
        save_mnemonic_seed(&main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_encrypted_wallet(
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
    let tmp_vault = get_encrypted_wallet(
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
        Some(1.1),
    )
    .await?;
    debug!("Ephemeral wallet funding tx: {tx:?}");

    info!("Get Walles");
    let assets_wallet = get_wallet_data(&tmp_vault.rgb_assets_descriptor_xpub, None).await?;
    let udas_wallet = get_wallet_data(&tmp_vault.rgb_udas_descriptor_xpub, None).await?;

    info!("Check Asset Vault");
    let mut assets_vault_details = get_assets_vault(
        &tmp_vault.rgb_assets_descriptor_xpub,
        &tmp_vault.rgb_udas_descriptor_xpub,
    )
    .await?;

    info!("Check Main Asset Vault");
    if assets_vault_details.assets_output.is_none() {
        info!("Missing an asset UTXO in vault. Funding vault...");
        assets_vault_details = fund_vault(
            &tmp_vault.btc_descriptor_xprv,
            &tmp_vault.btc_change_descriptor_xprv,
            &assets_wallet.address,
            &udas_wallet.address,
            546,
            546,
            Some(3.0),
        )
        .await?;
        debug!("Fund vault details: {assets_vault_details:#?}");
    }

    info!("Create fungible");
    let issued_asset = &create_asset(
        TICKER,
        NAME,
        PRECISION,
        SUPPLY,
        &assets_vault_details.assets_output.clone().unwrap(),
    )?;

    let asset_data = serde_json::to_string_pretty(&issued_asset)?;
    debug!("Asset data: {asset_data}");

    info!("Import Asset");
    let imported_asset = import_asset(&issued_asset.genesis, udas_wallet.utxos)?;
    assert_eq!(issued_asset.asset_id, imported_asset.id, "Asset IDs match");

    info!("Get a Blinded UTXO");
    let blinded_utxo =
        get_blinded_utxo(&assets_vault_details.assets_change_output.clone().unwrap())?;
    debug!("Blinded UTXO: {:?}", blinded_utxo);

    let transfers = vec![AssetTransfer {
        asset_contract: issued_asset.genesis.to_string(),
        asset_utxo: AssetUtxo {
            outpoint: OutPoint::from_str(&assets_vault_details.assets_output.unwrap())?,
            terminal_derivation: "/0/0".to_string(),
            commitment: "".to_string(),
        },
        asset_amount: SUPPLY,
        change_utxo: assets_vault_details.assets_change_output.unwrap(),
        beneficiaries: vec![format!("{}@{}", 10, blinded_utxo.conceal)],
    }];

    info!("Transfer asset");
    // Regtest workaround to sync transaction in electrs
    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);
    let resp = transfer_assets(TransfersRequest {
        descriptor_xpub: tmp_vault.rgb_assets_descriptor_xpub,
        transfers,
    })
    .await?;
    debug!("Transfer response: {:#?}", &resp);

    info!("Accept transfer");
    for transfer in resp.transfers {
        let accept_details = accept_transfer(
            &transfer.consignment,
            &blinded_utxo.blinding,
            &blinded_utxo.utxo.to_string(),
        )
        .await?;
        debug!("Accept response: {:#?}", &accept_details);
        assert_eq!(accept_details.id, issued_asset.asset_id, "RGB IDs match");
    }

    Ok(())
}

#[tokio::test]
async fn allow_transfer_one_asset_to_many_beneficiaries() -> Result<()> {
    let _ = pretty_env_logger::try_init();
    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let main_mnemonic_data =
        save_mnemonic_seed(&main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_encrypted_wallet(
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
    let tmp_vault = get_encrypted_wallet(
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
        Some(1.1),
    )
    .await?;
    debug!("Ephemeral wallet funding tx: {tx:?}");

    info!("Get Walles");
    let assets_wallet = get_wallet_data(&tmp_vault.rgb_assets_descriptor_xpub, None).await?;
    let udas_wallet = get_wallet_data(&tmp_vault.rgb_udas_descriptor_xpub, None).await?;

    info!("Check Asset Vault");
    let mut assets_vault_details = get_assets_vault(
        &tmp_vault.rgb_assets_descriptor_xpub,
        &tmp_vault.rgb_udas_descriptor_xpub,
    )
    .await?;

    info!("Check Main Asset Vault");
    if assets_vault_details.assets_output.is_none() {
        info!("Missing an asset UTXO in vault. Funding vault...");
        assets_vault_details = fund_vault(
            &tmp_vault.btc_descriptor_xprv,
            &tmp_vault.btc_change_descriptor_xprv,
            &assets_wallet.address,
            &udas_wallet.address,
            546,
            546,
            Some(3.0),
        )
        .await?;
        debug!("Fund vault details: {assets_vault_details:#?}");
    }

    info!("Create fungible");
    let issued_asset = &create_asset(
        TICKER,
        NAME,
        PRECISION,
        SUPPLY,
        &assets_vault_details.assets_output.clone().unwrap(),
    )?;

    let asset_data = serde_json::to_string_pretty(&issued_asset)?;
    debug!("Asset data: {asset_data}");

    info!("Import Asset");
    let imported_asset = import_asset(&issued_asset.genesis, udas_wallet.utxos)?;
    assert_eq!(issued_asset.asset_id, imported_asset.id, "Asset IDs match");

    info!("Get a Blinded UTXO");
    let blinded_utxo1 =
        get_blinded_utxo(&assets_vault_details.assets_change_output.clone().unwrap())?;
    debug!("Blinded UTXO 1: {:?}", blinded_utxo1.clone());

    let blinded_utxo2 =
        get_blinded_utxo(&assets_vault_details.assets_change_output.clone().unwrap())?;
    debug!("Blinded UTXO 2: {:?}", blinded_utxo2.clone());

    let transfers = vec![AssetTransfer {
        asset_contract: issued_asset.genesis.to_string(),
        asset_utxo: AssetUtxo {
            outpoint: OutPoint::from_str(&assets_vault_details.assets_output.unwrap())?,
            terminal_derivation: "/0/0".to_string(),
            commitment: "".to_string(),
        },
        asset_amount: SUPPLY,
        change_utxo: assets_vault_details.assets_change_output.unwrap(),
        beneficiaries: vec![
            format!("{}@{}", 10, blinded_utxo1.conceal.clone()),
            format!("{}@{}", 20, blinded_utxo2.conceal.clone()),
        ],
    }];

    let beneficiaries = BTreeMap::from([
        (blinded_utxo1.conceal.to_string(), blinded_utxo1),
        (blinded_utxo2.conceal.to_string(), blinded_utxo2),
    ]);

    info!("Transfer asset");
    // Regtest workaround to sync transaction in electrs
    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);
    let resp = transfer_assets(TransfersRequest {
        descriptor_xpub: tmp_vault.rgb_assets_descriptor_xpub,
        transfers,
    })
    .await?;
    debug!("Transfer response: {:#?}", &resp);

    info!("Accept transfer");
    for transfer in resp.transfers {
        for beneficiary in transfer.beneficiaries {
            if beneficiaries.contains_key(&beneficiary) {
                let reveal = beneficiaries
                    .get(&beneficiary)
                    .expect("Beneficiary not found in transition");
                let accept_details = accept_transfer(
                    &transfer.consignment,
                    &reveal.blinding,
                    &reveal.utxo.to_string(),
                )
                .await?;
                debug!("Accept response: {:#?}", &accept_details);
                assert_eq!(accept_details.id, issued_asset.asset_id, "RGB IDs match");
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn allow_transfer_assets_to_one_beneficiary() -> Result<()> {
    let _ = pretty_env_logger::try_init();
    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let main_mnemonic_data =
        save_mnemonic_seed(&main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_encrypted_wallet(
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
    let tmp_vault = get_encrypted_wallet(
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
        Some(1.1),
    )
    .await?;
    debug!("Ephemeral wallet funding tx: {tx:?}");

    info!("Get Walles");
    let assets_wallet = get_wallet_data(&tmp_vault.rgb_assets_descriptor_xpub, None).await?;
    let udas_wallet = get_wallet_data(&tmp_vault.rgb_udas_descriptor_xpub, None).await?;

    info!("Check Asset Vault");
    let mut assets_vault_details = get_assets_vault(
        &tmp_vault.rgb_assets_descriptor_xpub,
        &tmp_vault.rgb_udas_descriptor_xpub,
    )
    .await?;

    info!("Check Main Asset Vault");
    if assets_vault_details.assets_output.is_none() {
        info!("Missing an asset UTXO in vault. Funding vault...");
        assets_vault_details = fund_vault(
            &tmp_vault.btc_descriptor_xprv,
            &tmp_vault.btc_change_descriptor_xprv,
            &assets_wallet.address,
            &udas_wallet.address,
            546,
            546,
            Some(3.0),
        )
        .await?;
        debug!("Fund vault details: {assets_vault_details:#?}");
    }

    info!("Create fungible");
    let issued_asset = &create_asset(
        TICKER,
        NAME,
        PRECISION,
        SUPPLY,
        &assets_vault_details.assets_output.clone().unwrap(),
    )?;
    info!("Create fungible");
    let issued_asset2 = &create_asset(
        TICKER,
        NAME,
        PRECISION,
        SUPPLY,
        &assets_vault_details.assets_output.clone().unwrap(),
    )?;

    let issued_assets = vec![
        issued_asset.asset_id.clone(),
        issued_asset2.asset_id.clone(),
    ];

    let asset_data = serde_json::to_string_pretty(&issued_asset)?;
    debug!("Asset data: {asset_data}");

    info!("Import Asset");
    let imported_asset = import_asset(&issued_asset.genesis, udas_wallet.utxos)?;
    assert_eq!(issued_asset.asset_id, imported_asset.id, "Asset IDs match");

    info!("Get a Blinded UTXO");
    let blinded_utxo1 =
        get_blinded_utxo(&assets_vault_details.udas_change_output.clone().unwrap())?;
    debug!("Blinded UTXO 1: {:?}", blinded_utxo1.clone());

    let transfers = vec![
        AssetTransfer {
            asset_contract: issued_asset.genesis.to_string(),
            asset_utxo: AssetUtxo {
                outpoint: OutPoint::from_str(&assets_vault_details.assets_output.clone().unwrap())?,
                terminal_derivation: "/0/0".to_string(),
                commitment: "".to_string(),
            },
            asset_amount: SUPPLY,
            change_utxo: assets_vault_details.assets_change_output.clone().unwrap(),
            beneficiaries: vec![format!("{}@{}", 10, blinded_utxo1.conceal.clone())],
        },
        AssetTransfer {
            asset_contract: issued_asset2.genesis.to_string(),
            asset_utxo: AssetUtxo {
                outpoint: OutPoint::from_str(&assets_vault_details.assets_output.unwrap())?,
                terminal_derivation: "/0/0".to_string(),
                commitment: "".to_string(),
            },
            asset_amount: SUPPLY,
            change_utxo: assets_vault_details.assets_change_output.clone().unwrap(),
            beneficiaries: vec![format!("{}@{}", 10, blinded_utxo1.conceal.clone())],
        },
    ];

    let beneficiaries = BTreeMap::from([(blinded_utxo1.conceal.to_string(), blinded_utxo1)]);

    info!("Transfer asset");
    // Regtest workaround to sync transaction in electrs
    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);
    let resp = transfer_assets(TransfersRequest {
        descriptor_xpub: tmp_vault.rgb_assets_descriptor_xpub,
        transfers,
    })
    .await?;
    debug!("Transfer response: {:#?}", &resp);

    info!("Accept transfer");
    for transfer in resp.transfers {
        for beneficiary in transfer.beneficiaries {
            if beneficiaries.contains_key(&beneficiary) {
                let reveal = beneficiaries
                    .get(&beneficiary)
                    .expect("Beneficiary not found in transition");
                let accept_details = accept_transfer(
                    &transfer.consignment,
                    &reveal.blinding,
                    &reveal.utxo.to_string(),
                )
                .await?;
                debug!("Accept response: {:#?}", &accept_details);
                assert!(issued_assets.contains(&accept_details.id), "RGB IDs match");
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn allow_transfer_assets_to_many_beneficiary() -> Result<()> {
    let _ = pretty_env_logger::try_init();
    let network = get_network()?;
    info!("Asset test on {network}");

    info!("Import wallets");
    let main_mnemonic = env::var("TEST_WALLET_SEED")?;
    let main_mnemonic_data =
        save_mnemonic_seed(&main_mnemonic, ENCRYPTION_PASSWORD, SEED_PASSWORD)?;

    let main_vault = get_encrypted_wallet(
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
    let tmp_vault = get_encrypted_wallet(
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
        Some(1.1),
    )
    .await?;
    debug!("Ephemeral wallet funding tx: {tx:?}");

    info!("Get Walles");
    let assets_wallet = get_wallet_data(&tmp_vault.rgb_assets_descriptor_xpub, None).await?;
    let udas_wallet = get_wallet_data(&tmp_vault.rgb_udas_descriptor_xpub, None).await?;

    info!("Check Asset Vault");
    let mut assets_vault_details = get_assets_vault(
        &tmp_vault.rgb_assets_descriptor_xpub,
        &tmp_vault.rgb_udas_descriptor_xpub,
    )
    .await?;

    info!("Check Main Asset Vault");
    if assets_vault_details.assets_output.is_none() {
        info!("Missing an asset UTXO in vault. Funding vault...");
        assets_vault_details = fund_vault(
            &tmp_vault.btc_descriptor_xprv,
            &tmp_vault.btc_change_descriptor_xprv,
            &assets_wallet.address,
            &udas_wallet.address,
            546,
            546,
            Some(3.0),
        )
        .await?;
        debug!("Fund vault details: {assets_vault_details:#?}");
    }

    info!("Create fungible");
    let issued_asset = &create_asset(
        TICKER,
        NAME,
        PRECISION,
        SUPPLY,
        &assets_vault_details.assets_output.clone().unwrap(),
    )?;
    info!("Create fungible");
    let issued_asset2 = &create_asset(
        TICKER,
        NAME,
        PRECISION,
        SUPPLY,
        &assets_vault_details.assets_output.clone().unwrap(),
    )?;

    let issued_assets = vec![
        issued_asset.asset_id.clone(),
        issued_asset2.asset_id.clone(),
    ];

    let asset_data = serde_json::to_string_pretty(&issued_asset)?;
    debug!("Asset data: {asset_data}");

    info!("Import Asset");
    let imported_asset = import_asset(&issued_asset.genesis, udas_wallet.utxos)?;
    assert_eq!(issued_asset.asset_id, imported_asset.id, "Asset IDs match");

    info!("Get a Blinded UTXO");
    let blinded_utxo1 =
        get_blinded_utxo(&assets_vault_details.udas_change_output.clone().unwrap())?;
    debug!("Blinded UTXO 1: {:?}", blinded_utxo1.clone());

    info!("Get a Blinded UTXO");
    let blinded_utxo2 =
        get_blinded_utxo(&assets_vault_details.udas_change_output.clone().unwrap())?;
    debug!("Blinded UTXO 2: {:?}", blinded_utxo1.clone());

    let transfers = vec![
        AssetTransfer {
            asset_contract: issued_asset.genesis.to_string(),
            asset_utxo: AssetUtxo {
                outpoint: OutPoint::from_str(&assets_vault_details.assets_output.clone().unwrap())?,
                terminal_derivation: "/0/0".to_string(),
                commitment: "".to_string(),
            },
            asset_amount: SUPPLY,
            change_utxo: assets_vault_details.assets_change_output.clone().unwrap(),
            beneficiaries: vec![
                format!("{}@{}", 10, blinded_utxo1.conceal.clone()),
                format!("{}@{}", 20, blinded_utxo2.conceal.clone()),
            ],
        },
        AssetTransfer {
            asset_contract: issued_asset2.genesis.to_string(),
            asset_utxo: AssetUtxo {
                outpoint: OutPoint::from_str(&assets_vault_details.assets_output.unwrap())?,
                terminal_derivation: "/0/0".to_string(),
                commitment: "".to_string(),
            },
            asset_amount: SUPPLY,
            change_utxo: assets_vault_details.assets_change_output.clone().unwrap(),
            beneficiaries: vec![
                format!("{}@{}", 10, blinded_utxo1.conceal.clone()),
                format!("{}@{}", 20, blinded_utxo2.conceal.clone()),
            ],
        },
    ];

    let beneficiaries = BTreeMap::from([(blinded_utxo1.conceal.to_string(), blinded_utxo1)]);

    info!("Transfer asset");
    // Regtest workaround to sync transaction in electrs
    let five_secs = time::Duration::from_secs(5);
    thread::sleep(five_secs);
    let resp = transfer_assets(TransfersRequest {
        descriptor_xpub: tmp_vault.rgb_assets_descriptor_xpub,
        transfers,
    })
    .await?;
    debug!("Transfer response: {:#?}", &resp);

    info!("Accept transfer");
    for transfer in resp.transfers {
        for beneficiary in transfer.beneficiaries {
            if beneficiaries.contains_key(&beneficiary) {
                let reveal = beneficiaries
                    .get(&beneficiary)
                    .expect("Beneficiary not found in transition");
                let accept_details = accept_transfer(
                    &transfer.consignment,
                    &reveal.blinding,
                    &reveal.utxo.to_string(),
                )
                .await?;
                debug!("Accept response: {:#?}", &accept_details);
                assert!(issued_assets.contains(&accept_details.id), "RGB IDs match");
            }
        }
    }
    Ok(())
}
