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

pub async fn retrieve_stock(sk: &str) -> Result<Stock, RgbPersistenceError> {
    let stock = retrieve_rgb_stock(sk, ASSETS_STOCK)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveStock(op.to_string()))?;

    Ok(stock)
}

pub async fn retrieve_transfers(sk: &str) -> Result<RgbTransfers, RgbPersistenceError> {
    let rgb_account = retrieve_rgb_transfers(sk, ASSETS_TRANSFERS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbTransfers(op.to_string()))?;

    Ok(rgb_account)
}

pub async fn retrieve_account(sk: &str) -> Result<RgbAccount, RgbPersistenceError> {
    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbAccount(op.to_string()))?;

    Ok(rgb_account)
}

pub async fn retrieve_local_account(sk: &str) -> Result<LocalRgbAccount, RgbPersistenceError> {
    let rgb_account = retrieve_fork_wallets(sk, ASSETS_WALLETS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbAccountFork(op.to_string()))?;

    Ok(rgb_account)
}

pub async fn retrieve_stock_account(sk: &str) -> Result<(Stock, RgbAccount), RgbPersistenceError> {
    Ok((retrieve_stock(sk).await?, retrieve_account(sk).await?))
}

pub async fn retrieve_stock_transfers(
    sk: &str,
) -> Result<(Stock, RgbTransfers), RgbPersistenceError> {
    Ok((retrieve_stock(sk).await?, retrieve_transfers(sk).await?))
}

pub async fn retrieve_stock_account_transfers(
    sk: &str,
) -> Result<(Stock, RgbAccount, RgbTransfers), RgbPersistenceError> {
    Ok((
        retrieve_stock(sk).await?,
        retrieve_account(sk).await?,
        retrieve_transfers(sk).await?,
    ))
}

pub async fn store_stock(sk: &str, stock: Stock) -> Result<(), RgbPersistenceError> {
    store_rgb_stock(sk, ASSETS_STOCK, &stock)
        .await
        .map_err(|op| RgbPersistenceError::WriteStock(op.to_string()))
}

pub async fn store_transfers(sk: &str, transfers: RgbTransfers) -> Result<(), RgbPersistenceError> {
    store_rgb_transfer(sk, ASSETS_TRANSFERS, &transfers)
        .await
        .map_err(|op| RgbPersistenceError::WriteRgbTransfers(op.to_string()))
}

pub async fn store_account(sk: &str, account: RgbAccount) -> Result<(), RgbPersistenceError> {
    store_wallets(sk, ASSETS_WALLETS, &account)
        .await
        .map_err(|op| RgbPersistenceError::WriteRgbAccount(op.to_string()))
}

pub async fn store_local_account(sk: &str, changes: Vec<u8>) -> Result<(), RgbPersistenceError> {
    store_fork_wallets(sk, ASSETS_WALLETS, &changes)
        .await
        .map_err(|op| RgbPersistenceError::WriteRgbAccountFork(op.to_string()))
}

pub async fn store_stock_account(
    sk: &str,
    stock: Stock,
    account: RgbAccount,
) -> Result<(), RgbPersistenceError> {
    store_stock(sk, stock).await?;
    store_account(sk, account).await
}

pub async fn store_stock_transfers(
    sk: &str,
    stock: Stock,
    transfers: RgbTransfers,
) -> Result<(), RgbPersistenceError> {
    store_stock(sk, stock).await?;
    store_transfers(sk, transfers).await
}

pub async fn store_stock_account_transfers(
    sk: &str,
    stock: Stock,
    account: RgbAccount,
    transfers: RgbTransfers,
) -> Result<(), RgbPersistenceError> {
    store_stock(sk, stock).await?;
    store_account(sk, account).await?;
    store_transfers(sk, transfers).await
}
