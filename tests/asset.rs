use bitmask_core::{get_vault, get_wallet_data, import_asset, save_mnemonic_seed};

const MNEMONIC: &str =
    "swing rose forest coral approve giggle public liar brave piano sound spirit";
const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

const ASSET: &str = "rgb1g2antx89ypjuat7jdth35d8xgqserckrhj9elkrhxhjhxch8sxqqguzmh6"; // BUX

/// Test asset import
#[tokio::test]
async fn asset_import() {
    // Import wallet
    let mnemonic_data = save_mnemonic_seed(
        MNEMONIC.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    )
    .unwrap();

    let encrypted_descriptors =
        serde_json::to_string(&mnemonic_data.serialized_encrypted_message).unwrap();

    // Get vault properties
    let vault = get_vault(ENCRYPTION_PASSWORD.to_owned(), encrypted_descriptors).unwrap();

    let asset = import_asset(
        vault.rgb_tokens_descriptor.clone(),
        Some(ASSET.to_owned()),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(asset.id, ASSET);

    // Get wallet data
    let wallet = get_wallet_data(vault.rgb_tokens_descriptor.clone(), None)
        .await
        .unwrap();

    // Parse wallet data
    assert_eq!(wallet.transactions, vec![]);
}
