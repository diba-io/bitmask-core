use amplify::confinement::{Confined, U32};
use anyhow::Result;
use postcard::{from_bytes, to_allocvec};
use rgbstd::persistence::Stock;
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::carbonado::{retrieve, store};
use crate::rgb::constants::RGB_STRICT_TYPE_VERSION;
use crate::rgb::structs::RgbAccount;

pub async fn store_stock(sk: &str, name: &str, stock: &Stock) -> Result<()> {
    let data = stock.to_strict_serialized::<U32>()?;
    store(sk, name, &data, Some(RGB_STRICT_TYPE_VERSION.to_vec())).await
}

pub async fn retrieve_stock(sk: &str, name: &str) -> Result<Stock> {
    let (_, data) = retrieve(sk, name).await.unwrap_or_default();
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
    store(sk, name, &data, Some(RGB_STRICT_TYPE_VERSION.to_vec())).await
}

pub async fn retrieve_wallets(sk: &str, name: &str) -> Result<RgbAccount> {
    let (_, data) = retrieve(sk, name).await.unwrap_or_default();
    if data.is_empty() {
        Ok(RgbAccount::default())
    } else {
        let rgb_wallets = from_bytes(&data)?;
        Ok(rgb_wallets)
    }
}
