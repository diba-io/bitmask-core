use amplify::confinement::{Confined, U32};
use anyhow::Result;
use postcard::{from_bytes, to_allocvec};
use rgbstd::{persistence::Stock, stl::LIB_ID_RGB};
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::{
    carbonado::{retrieve, store},
    rgb::{constants::RGB_STRICT_TYPE_VERSION, structs::RgbAccount},
};

pub async fn store_stock(sk: &str, name: &str, stock: &Stock) -> Result<()> {
    let data = stock.to_strict_serialized::<U32>()?;
    store(
        sk,
        &format!("{name}/{LIB_ID_RGB}"),
        &data,
        false,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
}

pub async fn force_store_stock(sk: &str, name: &str, stock: &Stock) -> Result<()> {
    let data = stock.to_strict_serialized::<U32>()?;
    store(
        sk,
        &format!("{name}/{LIB_ID_RGB}"),
        &data,
        true,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
}

pub async fn retrieve_stock(sk: &str, name: &str) -> Result<Stock> {
    let (data, _) = retrieve(sk, &format!("{name}/{LIB_ID_RGB}"))
        .await
        .unwrap_or_default();
    if data.is_empty() {
        Ok(Stock::default())
    } else {
        let confined = Confined::try_from_iter(data)?;
        let stock = Stock::from_strict_serialized::<U32>(confined)?;

        Ok(stock)
    }
}

pub async fn store_wallets(sk: &str, name: &str, rgb_wallets: &RgbAccount) -> Result<()> {
    let data = to_allocvec(rgb_wallets)?;
    store(
        sk,
        &format!("{name}/{LIB_ID_RGB}"),
        &data,
        false,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
}

pub async fn retrieve_wallets(sk: &str, name: &str) -> Result<RgbAccount> {
    let (data, _) = retrieve(sk, &format!("{name}/{LIB_ID_RGB}"))
        .await
        .unwrap_or_default();
    if data.is_empty() {
        Ok(RgbAccount::default())
    } else {
        let rgb_wallets = from_bytes(&data)?;
        Ok(rgb_wallets)
    }
}
