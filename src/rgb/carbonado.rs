use amplify::confinement::{Confined, U32};
use anyhow::Result;
use autosurgeon::{hydrate, reconcile};
use postcard::{from_bytes, to_allocvec};
use rgbstd::{persistence::Stock, stl::LIB_ID_RGB};
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::rgb::crdt::LocalRgbAccount;
use crate::rgb::crdt::RawRgbAccount;
use crate::rgb::structs::RgbTransfers;
use crate::{
    carbonado::{retrieve, store},
    rgb::{constants::RGB_STRICT_TYPE_VERSION, structs::RgbAccount},
};

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum StorageError {
    /// File '{0}' retrieve causes error. {1}
    FileRetrieve(String, String),
    /// File '{0}' write causes error. {1}
    WriteRetrieve(String, String),
    /// Changes '{0}' retrieve causes error. {1}
    ChangesRetrieve(String, String),
    /// Changes '{0}' write causes error. {1}
    ChangesWrite(String, String),
    /// Fork '{0}' write causes error. {1}
    ForkWrite(String, String),
    /// Merge '{0}' write causes error. {1}
    MergeWrite(String, String),
    /// Retrieve '{0}' strict-encoding causes error. {1}
    StrictRetrieve(String, String),
    /// Write '{0}' strict-encoding causes error. {1}
    StrictWrite(String, String),
    /// Retrieve '{0}' serialize causes error. {1}
    SerializeRetrieve(String, String),
    /// Write '{0}' serialize causes error. {1}
    SerializeWrite(String, String),
    /// Retrieve '{0}' carbonado causes error. {1}
    CarbonadoRetrieve(String, String),
    /// Write '{0}' carbonado causes error. {1}
    CarbonadoWrite(String, String),
    /// Reconcile '{0}' causes error. {1}
    Reconcile(String, String),
}

pub async fn store_stock(sk: &str, name: &str, stock: &Stock) -> Result<(), StorageError> {
    let data = stock
        .to_strict_serialized::<U32>()
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    store(
        sk,
        &format!("{hashed_name}.c15"),
        &data,
        false,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn store_wallets(
    sk: &str,
    name: &str,
    rgb_wallets: &RgbAccount,
) -> Result<(), StorageError> {
    let data = to_allocvec(rgb_wallets)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    store(
        sk,
        &format!("{hashed_name}.c15"),
        &data,
        false,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn store_transfers(
    sk: &str,
    name: &str,
    rgb_transfers: &RgbTransfers,
) -> Result<(), StorageError> {
    let data = to_allocvec(rgb_transfers)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    store(
        sk,
        &format!("{hashed_name}.c15"),
        &data,
        true,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn retrieve_stock(sk: &str, name: &str) -> Result<Stock, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(Stock::default())
    } else {
        let confined = Confined::try_from_iter(data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        let stock = Stock::from_strict_serialized::<U32>(confined)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

        Ok(stock)
    }
}

pub async fn retrieve_wallets(sk: &str, name: &str) -> Result<RgbAccount, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbAccount::default())
    } else {
        let rgb_wallets = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        Ok(rgb_wallets)
    }
}

pub async fn retrieve_transfers(sk: &str, name: &str) -> Result<RgbTransfers, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbTransfers::default())
    } else {
        let rgb_wallets = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        Ok(rgb_wallets)
    }
}

pub async fn store_fork_wallets(sk: &str, name: &str, changes: &[u8]) -> Result<(), StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    // let mut original_version = automerge::AutoCommit::new();
    // let (main_bytes, _) = retrieve(sk, &main_name, vec![])
    //     .await
    //     .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    // let original: RgbAccount = from_bytes(&main_bytes)
    //     .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

    // let raw_data = RawRgbAccount::from(original);
    // reconcile(&mut original_version, raw_data)
    //     .map_err(|op| StorageError::Reconcile(name.to_string(), op.to_string()))?;

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
    let merged: RgbAccount = RgbAccount::from(raw_merged.clone());

    let mut latest_version = automerge::AutoCommit::new();
    reconcile(&mut latest_version, raw_merged)
        .map_err(|op| StorageError::WriteRetrieve(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&merged)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    store(
        sk,
        main_name,
        &data,
        true,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

pub async fn retrieve_fork_wallets(sk: &str, name: &str) -> Result<LocalRgbAccount, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (data, _) = retrieve(sk, main_name, vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(LocalRgbAccount {
            doc: automerge::AutoCommit::new().save(),
            rgb_account: RgbAccount::default(),
        })
    } else {
        let mut original_version = automerge::AutoCommit::new();
        let rgb_account: RgbAccount = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

        let raw_rgb_account = RawRgbAccount::from(rgb_account.clone());
        reconcile(&mut original_version, raw_rgb_account.clone())
            .map_err(|op| StorageError::Reconcile(name.to_string(), op.to_string()))?;

        let mut fork_version = original_version.fork();
        let original_version = fork_version.save();

        store(
            sk,
            original_name,
            &original_version,
            true,
            Some(RGB_STRICT_TYPE_VERSION.to_vec()),
        )
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

        Ok(LocalRgbAccount {
            doc: fork_version.save(),
            rgb_account,
        })
    }
}
