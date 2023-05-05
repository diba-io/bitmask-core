use amplify::confinement::{Confined, U32};
use anyhow::Result;
use rgbstd::persistence::Stock;
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::{
    carbonado::{retrieve, store},
    info,
};

pub async fn store_stock(sk: &str, name: &str, stock: &Stock) -> Result<()> {
    let data = stock.to_strict_serialized::<U32>()?;

    store(sk, name, &data).await
}

pub async fn retrieve_stock(sk: &str, name: &str) -> Result<Stock> {
    let data = retrieve(sk, name).await.unwrap_or_default();
    info!("data: ");
    if data.is_empty() {
        info!("data is empty");
        Ok(Stock::default())
    } else {
        info!("data is not empty");
        let confined = Confined::try_from_iter(data)?;
        let stock = Stock::from_strict_serialized::<U32>(confined)?;

        Ok(stock)
    }
}
