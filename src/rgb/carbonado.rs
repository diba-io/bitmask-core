use amplify::confinement::{Confined, U32};
use anyhow::Result;
use postcard::{from_bytes, to_allocvec};
use rgbstd::{persistence::Stock, stl::LIB_ID_RGB};
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::rgb::structs::RgbTransfers;
use crate::{
    carbonado::{retrieve, store},
    rgb::{constants::RGB_STRICT_TYPE_VERSION, structs::RgbAccount},
};

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum StorageError {
    /// Retrieve '{0}' strict-encoding causes error. {1}
    StrictRetrive(String, String),
    /// Write '{0}' strict-encoding causes error. {1}
    StrictWrite(String, String),
    /// Retrieve '{0}' carbonado causes error. {1}
    CarbonadoRetrive(String, String),
    /// Write '{0}' carbonado causes error. {1}
    CarbonadoWrite(String, String),
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

pub async fn force_store_stock(sk: &str, name: &str, stock: &Stock) -> Result<(), StorageError> {
    let data = stock
        .to_strict_serialized::<U32>()
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    store(
        sk,
        &hashed_name,
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
        .map_err(|op| StorageError::CarbonadoRetrive(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(Stock::default())
    } else {
        let confined = Confined::try_from_iter(data)
            .map_err(|op| StorageError::StrictRetrive(name.to_string(), op.to_string()))?;
        let stock = Stock::from_strict_serialized::<U32>(confined)
            .map_err(|op| StorageError::StrictRetrive(name.to_string(), op.to_string()))?;

        Ok(stock)
    }
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

pub async fn force_store_wallets(
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
        true,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn retrieve_wallets(sk: &str, name: &str) -> Result<RgbAccount, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrive(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbAccount::default())
    } else {
        let rgb_wallets = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrive(name.to_string(), op.to_string()))?;
        Ok(rgb_wallets)
    }
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

pub async fn retrieve_transfers(sk: &str, name: &str) -> Result<RgbTransfers, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrive(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbTransfers::default())
    } else {
        let rgb_wallets = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrive(name.to_string(), op.to_string()))?;
        Ok(rgb_wallets)
    }
}
