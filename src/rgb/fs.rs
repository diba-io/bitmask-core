use rgbstd::persistence::Stock;

use super::crdt::LocalRgbOffers;
use super::swap::{RgbBids, RgbOffers};
use crate::constants::storage_keys::{
    ASSETS_BIDS, ASSETS_OFFERS, ASSETS_STOCK, ASSETS_TRANSFERS, ASSETS_WALLETS, MARKETPLACE_OFFERS,
};

use crate::rgb::{
    carbonado::{
        retrieve_bids as retrieve_rgb_bids, retrieve_fork_wallets,
        retrieve_offers as retrieve_rgb_offers,
        retrieve_public_offers as retrieve_rgb_public_offers, retrieve_stock as retrieve_rgb_stock,
        retrieve_transfers as retrieve_rgb_transfers, retrieve_wallets,
        store_bids as store_rgb_bids, store_fork_wallets, store_offers as store_rgb_offers,
        store_public_offers as store_rgb_public_offers, store_stock as store_rgb_stock,
        store_transfers as store_rgb_transfer, store_wallets,
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
    // Retrieve Offers Error. {0}
    RetrieveRgbOffers(String),
    // Retrieve Bids Error. {0}
    RetrieveRgbBids(String),
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

pub async fn retrieve_public_offers() -> Result<LocalRgbOffers, RgbPersistenceError> {
    let stock = retrieve_rgb_public_offers(MARKETPLACE_OFFERS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbOffers(op.to_string()))?;

    Ok(stock)
}

pub async fn retrieve_offers(sk: &str) -> Result<RgbOffers, RgbPersistenceError> {
    let offers = retrieve_rgb_offers(sk, ASSETS_OFFERS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbOffers(op.to_string()))?;

    Ok(offers)
}

pub async fn retrieve_bids(sk: &str) -> Result<RgbBids, RgbPersistenceError> {
    let bids = retrieve_rgb_bids(sk, ASSETS_BIDS)
        .await
        .map_err(|op| RgbPersistenceError::RetrieveRgbBids(op.to_string()))?;

    Ok(bids)
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

pub async fn store_offers(sk: &str, rgb_offers: RgbOffers) -> Result<(), RgbPersistenceError> {
    store_rgb_offers(sk, ASSETS_OFFERS, &rgb_offers)
        .await
        .map_err(|op| RgbPersistenceError::WriteStock(op.to_string()))
}

pub async fn store_bids(sk: &str, rgb_bids: RgbBids) -> Result<(), RgbPersistenceError> {
    store_rgb_bids(sk, ASSETS_BIDS, &rgb_bids)
        .await
        .map_err(|op| RgbPersistenceError::WriteStock(op.to_string()))
}

pub async fn store_public_offers(changes: Vec<u8>) -> Result<(), RgbPersistenceError> {
    store_rgb_public_offers(MARKETPLACE_OFFERS, &changes)
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
