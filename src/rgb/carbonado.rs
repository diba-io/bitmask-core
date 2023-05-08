use amplify::confinement::{Confined, U32};
use anyhow::Result;
use rgbstd::persistence::Stock;
use serde_json::to_vec;
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::carbonado::{retrieve, store};

use super::structs::RgbAccount;

pub async fn store_stock(sk: &str, name: &str, stock: &Stock) -> Result<()> {
    let data = stock.to_strict_serialized::<U32>()?;

    store(sk, name, &data).await
}

pub async fn retrieve_stock(sk: &str, name: &str) -> Result<Stock> {
    let data = retrieve(sk, name).await.unwrap_or_default();
    if data.is_empty() {
        Ok(Stock::default())
    } else {
        let confined = Confined::try_from_iter(data)?;
        let stock = Stock::from_strict_serialized::<U32>(confined)?;

        Ok(stock)
    }
}

pub async fn store_wallets(sk: &str, name: &str, rgb_wallets: &RgbAccount) -> Result<()> {
    let data = to_vec(rgb_wallets)?;
    store(sk, name, &data).await
}

pub async fn retrieve_wallets(sk: &str, name: &str) -> Result<RgbAccount> {
    let data = retrieve(sk, name).await.unwrap_or_default();
    if data.is_empty() {
        Ok(RgbAccount::default())
    } else {
        let rgb_wallets = serde_json::from_slice(&data)?;
        Ok(rgb_wallets)
    }
}
