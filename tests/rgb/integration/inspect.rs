#![cfg(not(target_arch = "wasm32"))]
use amplify::confinement::{Confined, U32};
use amplify::hex::ToHex;
use anyhow::Result;
use bitcoin::secp256k1::{PublicKey, SecretKey};
use bitmask_core::constants::storage_keys::{ASSETS_STOCK, ASSETS_WALLETS};
use bitmask_core::rgb::cambria::{ModelVersion, RgbAccountVersions};
use bitmask_core::rgb::inspect_contract;
use bitmask_core::rgb::structs::RgbAccountV1;
use bitmask_core::{
    bitcoin::save_mnemonic,
    constants::{switch_network, CARBONADO_ENDPOINT, NETWORK},
    structs::SecretString,
};
use rgbstd::persistence::Stock;
use rgbstd::stl::LIB_ID_RGB;
use strict_encoding::StrictDeserialize;

#[ignore = "Only for troubleshotting"]
#[tokio::test]
pub async fn inspect_contract_states() -> Result<()> {
    // 0. Switc network
    switch_network("bitcoin").await?;

    // 1. Retrieve all keys
    let wallet_a_keys =
        save_mnemonic(&SecretString("".to_string()), &SecretString("".to_string())).await?;
    let wallet_b_keys =
        save_mnemonic(&SecretString("".to_string()), &SecretString("".to_string())).await?;

    let contract_id = "";

    // 2. Extract Stock and RgbAccount (wallet A)
    let wallet_a_sk = &wallet_a_keys.private.nostr_prv;
    let mut stock = retrieve_stock(wallet_a_sk, ASSETS_STOCK).await?;
    let rgb_account = retrieve_account(wallet_a_sk, ASSETS_WALLETS).await?;

    println!("Wallet A");
    let contract = inspect_contract(&mut stock, rgb_account, contract_id).await?;
    println!(
        "Contract {} ({}): \n {:#?}",
        contract.name, contract.contract_id, contract.allocations
    );

    // 3. Extract Stock and RgbAccount (wallet B)
    let wallet_b_sk = &wallet_b_keys.private.nostr_prv;
    let mut stock = retrieve_stock(wallet_b_sk, ASSETS_STOCK).await?;
    let rgb_account = retrieve_account(wallet_b_sk, ASSETS_WALLETS).await?;

    println!("Wallet B");
    let contract = inspect_contract(&mut stock, rgb_account, contract_id).await?;
    println!(
        "Contract {} ({}): \n {:#?}",
        contract.name, contract.balance, contract.allocations
    );

    Ok(())
}

async fn retrieve_stock(sk_str: &str, name: &str) -> Result<Stock> {
    let sk = hex::decode(sk_str)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let final_name = format!("{hashed_name}.c15");
    let network = NETWORK.read().await.to_string();

    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let url = format!("{endpoint}/{pk}/{network}-{final_name}");

    let (data, _) = fetch(sk_str, url).await?;

    if data.is_empty() {
        Ok(Stock::default())
    } else {
        let confined = Confined::try_from_iter(data)?;
        let stock = Stock::from_strict_serialized::<U32>(confined)?;

        Ok(stock)
    }
}

async fn retrieve_account(sk_str: &str, name: &str) -> Result<RgbAccountV1> {
    let sk = hex::decode(sk_str)?;
    let secret_key = SecretKey::from_slice(&sk)?;
    let public_key = PublicKey::from_secret_key_global(&secret_key);
    let pk = public_key.to_hex();

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let final_name = format!("{hashed_name}.c15");
    let network = NETWORK.read().await.to_string();

    let endpoint = CARBONADO_ENDPOINT.read().await.to_string();
    let url = format!("{endpoint}/{pk}/{network}-{final_name}");

    let (data, metadata) = fetch(sk_str, url).await?;

    if data.is_empty() {
        Ok(RgbAccountV1::default())
    } else {
        let mut version: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        if let Some(metadata) = metadata {
            version.copy_from_slice(&metadata);
        }

        let rgb_wallets = RgbAccountVersions::from_bytes(data, version)?;
        Ok(rgb_wallets)
    }
}

async fn fetch(sk: &str, url: String) -> Result<(Vec<u8>, Option<[u8; 8]>)> {
    let sk = hex::decode(sk)?;
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await?;

    let bytes = resp.bytes().await?.to_vec();
    let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;

    Ok((decoded, header.metadata))
}
