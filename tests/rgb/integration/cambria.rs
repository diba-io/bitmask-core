#![cfg(not(target_arch = "wasm32"))]

use std::str::FromStr;

use amplify::default;
use autosurgeon::{hydrate, reconcile};
use bitcoin_30::bip32::ExtendedPubKey;
use bitmask_core::{
    bitcoin::new_mnemonic,
    carbonado::{retrieve, store},
    rgb::{
        carbonado::{
            cdrt_retrieve_wallets, cdrt_store_wallets, retrieve_transfers, retrieve_wallets,
            store_transfers, store_wallets, StorageError,
        },
        constants::{RGB_DEFAULT_NAME, RGB_OLDEST_VERSION, RGB_STRICT_TYPE_VERSION},
        crdt::{LocalRgbAccount, RawRgbAccount},
        structs::{RgbAccountV0, RgbAccountV1, RgbTransfersV0, RgbTransfersV1},
    },
    structs::SecretString,
};
use miniscript_crate::DescriptorPublicKey;
use postcard::to_allocvec;
use rgb::{RgbDescr, RgbWallet, Tapret};
use rgbstd::stl::LIB_ID_RGB;

#[tokio::test]
async fn migrate_rgb_acc_from_v0_to_v1() -> anyhow::Result<()> {
    let name = "migrate_rgb_acc_from_v0_to_v1.c15";

    let user_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let user_sk = &user_keys.private.nostr_prv;
    let watcher_pub = &user_keys.public.watcher_xpub;

    let xdesc = DescriptorPublicKey::from_str(watcher_pub)?;
    let mut v0 = RgbAccountV0::default();
    if let DescriptorPublicKey::XPub(xpub) = xdesc {
        let xpub = xpub.xkey;
        let xpub = ExtendedPubKey::from_str(&xpub.to_string())?;

        let rgb_wallet = RgbWallet::new(RgbDescr::Tapret(Tapret {
            xpub,
            taprets: default!(),
        }));

        v0.wallets.insert(RGB_DEFAULT_NAME.to_string(), rgb_wallet);
    }

    // Case 1: no version
    save_wallet_v0(user_sk, name, &v0, None).await?;

    let v1 = get_wallet_v1(user_sk, name).await?;
    assert_eq!(v0.wallets, v1.wallets);
    assert!(v1.hidden_contracts.is_empty());

    // Case 2: oldest version
    save_wallet_v0(user_sk, name, &v0, Some(RGB_OLDEST_VERSION.to_vec())).await?;

    let v1 = get_wallet_v1(user_sk, name).await?;
    assert_eq!(v0.wallets, v1.wallets);
    assert!(v1.hidden_contracts.is_empty());

    // Case 3: strict-type 1.6.x version
    save_wallet_v0(user_sk, name, &v0, Some(RGB_STRICT_TYPE_VERSION.to_vec())).await?;

    let v1 = get_wallet_v1(user_sk, name).await?;
    assert_eq!(v0.wallets, v1.wallets);
    assert!(v1.hidden_contracts.is_empty());

    // Case 4: v0
    save_wallet_v0(user_sk, name, &v0, Some(b"v0".to_vec())).await?;

    let v1 = get_wallet_v1(user_sk, name).await?;
    assert_eq!(v0.wallets, v1.wallets);
    assert!(v1.hidden_contracts.is_empty());

    // Case 5: Save v1
    save_wallet_v1(user_sk, name, v1).await?;

    Ok(())
}

#[tokio::test]
async fn migrate_local_rgb_acc_from_v0_to_v1() -> anyhow::Result<()> {
    let name = "migrate_local_rgb_acc_from_v0_to_v1.c15";
    let user_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let user_sk = &user_keys.private.nostr_prv;
    let watcher_pub = &user_keys.public.watcher_xpub;

    let xdesc = DescriptorPublicKey::from_str(watcher_pub)?;
    let mut v0 = RgbAccountV0::default();
    if let DescriptorPublicKey::XPub(xpub) = xdesc {
        let xpub = xpub.xkey;
        let xpub = ExtendedPubKey::from_str(&xpub.to_string())?;

        let rgb_wallet = RgbWallet::new(RgbDescr::Tapret(Tapret {
            xpub,
            taprets: default!(),
        }));

        v0.wallets.insert(RGB_DEFAULT_NAME.to_string(), rgb_wallet);
    }

    let mut local_copy = automerge::AutoCommit::new();
    let local_v0 = RawRgbAccount::from(v0);
    reconcile(&mut local_copy, local_v0.clone())?;

    // Case 1: no version
    save_wallet_changes_v0(user_sk, name, &local_copy.save(), None).await?;

    let v1 = get_wallet_v1_copy(user_sk, name).await?;
    let local_v1 = RawRgbAccount::from(v1.rgb_account);

    assert_eq!(local_v0.wallets, local_v1.wallets);
    assert!(local_v1.hidden_contracts.is_empty());

    // Case 2: oldest version
    save_wallet_changes_v0(
        user_sk,
        name,
        &local_copy.save(),
        Some(RGB_OLDEST_VERSION.to_vec()),
    )
    .await?;

    let v1 = get_wallet_v1_copy(user_sk, name).await?;
    let local_v1 = RawRgbAccount::from(v1.rgb_account);

    assert_eq!(local_v0.wallets, local_v1.wallets);
    assert!(local_v1.hidden_contracts.is_empty());

    // Case 3: strict-type 1.6.x version
    save_wallet_changes_v0(
        user_sk,
        name,
        &local_copy.save(),
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await?;

    let v1 = get_wallet_v1_copy(user_sk, name).await?;
    let local_v1 = RawRgbAccount::from(v1.rgb_account);

    assert_eq!(local_v0.wallets, local_v1.wallets);
    assert!(local_v1.hidden_contracts.is_empty());

    // Case 4: v0
    save_wallet_changes_v0(user_sk, name, &local_copy.save(), Some(b"v0".to_vec())).await?;

    let v1 = get_wallet_v1_copy(user_sk, name).await?;
    let local_v1 = RawRgbAccount::from(v1.rgb_account);

    assert_eq!(local_v0.wallets, local_v1.wallets);
    assert!(local_v1.hidden_contracts.is_empty());

    // Case 5: Save v1
    let v1 = get_wallet_v1_copy(user_sk, name).await?;
    let mut local_v1 = RawRgbAccount::from(v1.rgb_account);
    local_v1.hidden_contracts.push("test".into());

    reconcile(&mut local_copy, local_v1)?;
    save_wallet_changes_v1(user_sk, name, local_copy.save()).await?;

    Ok(())
}

#[tokio::test]
async fn migrate_rgb_transfer_from_v0_to_v1() -> anyhow::Result<()> {
    let name = "migrate_rgb_transfer_from_v0_to_v1.c15";

    let user_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let user_sk = &user_keys.private.nostr_prv;
    let v0 = RgbTransfersV0::default();

    // Case 1: no version
    save_transfers_v0(user_sk, name, &v0, None).await?;

    let v1 = get_transfers_v1(user_sk, name).await?;
    assert_eq!(v0.transfers.len(), v1.transfers.len());

    // Case 2: oldest version
    save_transfers_v0(user_sk, name, &v0, Some(RGB_OLDEST_VERSION.to_vec())).await?;

    let v1 = get_transfers_v1(user_sk, name).await?;
    assert_eq!(v0.transfers.len(), v1.transfers.len());

    // Case 3: strict-type 1.6.x version
    save_transfers_v0(user_sk, name, &v0, Some(RGB_STRICT_TYPE_VERSION.to_vec())).await?;

    let v1 = get_transfers_v1(user_sk, name).await?;
    assert_eq!(v0.transfers.len(), v1.transfers.len());

    // Case 4: v0
    save_transfers_v0(user_sk, name, &v0, Some(b"v0".to_vec())).await?;

    let v1 = get_transfers_v1(user_sk, name).await?;
    assert_eq!(v0.transfers.len(), v1.transfers.len());

    // Case 5: Save v1
    save_transfers_v1(user_sk, name, v1).await?;

    Ok(())
}

async fn save_wallet_v0(
    sk: &str,
    name: &str,
    rgb_wallets: &RgbAccountV0,
    metadata: Option<Vec<u8>>,
) -> Result<(), StorageError> {
    let data = to_allocvec(rgb_wallets)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    store(sk, &format!("{hashed_name}.c15"), &data, false, metadata)
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn save_wallet_changes_v0(
    sk: &str,
    name: &str,
    changes: &[u8],
    metadata: Option<Vec<u8>>,
) -> Result<(), StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (original_bytes, _) = retrieve(sk, original_name, vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    let mut original_version = automerge::AutoCommit::load(&original_bytes)
        .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

    let mut fork_version = automerge::AutoCommit::load(changes)
        .map_err(|op| StorageError::ChangesRetrieve(name.to_string(), op.to_string()))?;

    original_version
        .merge(&mut fork_version)
        .map_err(|op| StorageError::MergeWrite(name.to_string(), op.to_string()))?;

    let raw_merged: RawRgbAccount = hydrate(&original_version).unwrap();
    let merged: RgbAccountV1 = RgbAccountV1::from(raw_merged.clone());

    let mut latest_version = automerge::AutoCommit::new();
    reconcile(&mut latest_version, raw_merged)
        .map_err(|op| StorageError::FileWrite(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&merged)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    store(sk, main_name, &data, true, metadata)
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

async fn save_wallet_v1(
    sk: &str,
    name: &str,
    rgb_wallets: RgbAccountV1,
) -> Result<(), StorageError> {
    store_wallets(sk, name, &rgb_wallets).await
}

async fn get_wallet_v1(sk: &str, name: &str) -> Result<RgbAccountV1, StorageError> {
    retrieve_wallets(sk, name).await
}

async fn save_wallet_changes_v1(
    sk: &str,
    name: &str,
    changes: Vec<u8>,
) -> Result<(), StorageError> {
    cdrt_store_wallets(sk, name, &changes).await
}

async fn get_wallet_v1_copy(sk: &str, name: &str) -> Result<LocalRgbAccount, StorageError> {
    cdrt_retrieve_wallets(sk, name).await
}

async fn save_transfers_v0(
    sk: &str,
    name: &str,
    rgb_wallets: &RgbTransfersV0,
    metadata: Option<Vec<u8>>,
) -> Result<(), StorageError> {
    let data = to_allocvec(rgb_wallets)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    store(sk, &format!("{hashed_name}.c15"), &data, false, metadata)
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

async fn save_transfers_v1(
    sk: &str,
    name: &str,
    rgb_transfers: RgbTransfersV1,
) -> Result<(), StorageError> {
    store_transfers(sk, name, &rgb_transfers).await
}

async fn get_transfers_v1(sk: &str, name: &str) -> Result<RgbTransfersV1, StorageError> {
    retrieve_transfers(sk, name).await
}
