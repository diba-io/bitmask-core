use rgbstd::persistence::Stock;

use crate::constants::storage_keys::{ASSETS_STOCK, ASSETS_TRANSFERS, ASSETS_WALLETS};
use crate::rgb::{
    carbonado::{
        retrieve_fork_wallets, retrieve_stock as retrieve_rgb_stock,
        retrieve_transfers as retrieve_rgb_transfers, retrieve_wallets, store_fork_wallets,
        store_stock as store_rgb_stock, store_transfers as store_rgb_transfer, store_wallets,
    },
    crdt::LocalRgbAccount,
    structs::{RgbAccount, RgbTransfers},
};

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbPersistenceError {
    // Retrieve Stock Error. {0}
    RetrieveStock(String),
    // Retrieve RgbAccount Error. {0}
    RetrieveRgbAccount(String),
    // Retrieve RgbAccount (Fork) Error. {0}
    RetrieveRgbAccountFork(String),
    // Retrieve Transfers Error. {0}
    RetrieveRgbTransfers(String),
    // Store Stock Error. {0}
    WriteStock(String),
    // Store RgbAccount Error. {0}
    WriteRgbAccount(String),
    // Store RgbAccount (Fork) Error. {0}
    WriteRgbAccountFork(String),
    // Store Transfers Error. {0}
    WriteRgbTransfers(String),
}

pub async fn retrieve_stock() -> Result<Stock, RgbPersistenceError> {
    let stock = retrieve_rgb_stock(ASSETS_STOCK)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveStock(op.to_string()))?;

    Ok(stock)
}

pub async fn retrieve_transfers() -> Result<RgbTransfers, RgbPersistenceError> {
    let rgb_account = retrieve_rgb_transfers(ASSETS_TRANSFERS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbTransfers(op.to_string()))?;

    Ok(rgb_account)
}

pub async fn retrieve_account() -> Result<RgbAccount, RgbPersistenceError> {
    let rgb_account = retrieve_wallets(ASSETS_WALLETS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbAccount(op.to_string()))?;

    Ok(rgb_account)
}

pub async fn retrieve_local_account() -> Result<LocalRgbAccount, RgbPersistenceError> {
    let rgb_account = retrieve_fork_wallets(ASSETS_WALLETS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbAccountFork(op.to_string()))?;

    Ok(rgb_account)
}

pub async fn retrieve_stock_account() -> Result<(Stock, RgbAccount), RgbPersistenceError> {
    Ok((retrieve_stock().await?, retrieve_account().await?))
}

pub async fn retrieve_stock_transfers() -> Result<(Stock, RgbTransfers), RgbPersistenceError> {
    Ok((retrieve_stock().await?, retrieve_transfers().await?))
}

pub async fn retrieve_stock_account_transfers(
) -> Result<(Stock, RgbAccount, RgbTransfers), RgbPersistenceError> {
    Ok((
        retrieve_stock().await?,
        retrieve_account().await?,
        retrieve_transfers().await?,
    ))
}

pub async fn store_stock(stock: Stock) -> Result<(), RgbPersistenceError> {
    store_rgb_stock(ASSETS_STOCK, &stock)
        .await
        .map_err(|op| RgbPersistenceError::WriteStock(op.to_string()))
}

pub async fn store_transfers(transfers: RgbTransfers) -> Result<(), RgbPersistenceError> {
    store_rgb_transfer(ASSETS_TRANSFERS, &transfers)
        .await
        .map_err(|op| RgbPersistenceError::WriteRgbTransfers(op.to_string()))
}

pub async fn store_account(account: RgbAccount) -> Result<(), RgbPersistenceError> {
    store_wallets(ASSETS_WALLETS, &account)
        .await
        .map_err(|op| RgbPersistenceError::WriteRgbAccount(op.to_string()))
}

pub async fn store_local_account(changes: Vec<u8>) -> Result<(), RgbPersistenceError> {
    store_fork_wallets(ASSETS_WALLETS, &changes)
        .await
        .map_err(|op| RgbPersistenceError::WriteRgbAccountFork(op.to_string()))
}

pub async fn store_stock_account(
    stock: Stock,
    account: RgbAccount,
) -> Result<(), RgbPersistenceError> {
    store_stock(stock).await?;
    store_account(account).await
}

pub async fn store_stock_transfers(
    stock: Stock,
    transfers: RgbTransfers,
) -> Result<(), RgbPersistenceError> {
    store_stock(stock).await?;
    store_transfers(transfers).await
}

pub async fn store_stock_account_transfers(
    stock: Stock,
    account: RgbAccount,
    transfers: RgbTransfers,
) -> Result<(), RgbPersistenceError> {
    store_stock(stock).await?;
    store_account(account).await?;
    store_transfers(transfers).await
}
