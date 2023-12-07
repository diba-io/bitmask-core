use amplify::confinement::{Confined, U32};
use anyhow::Result;
use autosurgeon::{hydrate, reconcile};
use postcard::{from_bytes, to_allocvec};
use rgbstd::{persistence::Stock, stl::LIB_ID_RGB};
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::carbonado::{auctions_retrieve, auctions_store, marketplace_store};
use crate::rgb::crdt::{LocalRgbAccount, LocalRgbOffers, RawRgbAccount};

use crate::rgb::swap::{RgbBids, RgbOffers};
use crate::{
    carbonado::{marketplace_retrieve, retrieve, store},
    rgb::{
        cambria::{ModelVersion, RgbAccountVersions},
        constants::RGB_STRICT_TYPE_VERSION,
        crdt::LocalRgbOfferBid,
        structs::RgbAccountV1,
    },
};

use super::cambria::RgbtransferVersions;
use super::crdt::LocalRgbAuctions;
use super::structs::RgbTransfersV1;
use super::swap::{RgbAuctionSwaps, RgbBidSwap, RgbPublicSwaps};

const RGB_ACCOUNT_VERSION: [u8; 3] = *b"v10";
const RGB_TRANSFER_VERSION: [u8; 3] = *b"v10";

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum StorageError {
    /// File '{0}' retrieve causes error. {1}
    FileRetrieve(String, String),
    /// File '{0}' write causes error. {1}
    FileWrite(String, String),
    /// Changes '{0}' retrieve causes error. {1}
    ChangesRetrieve(String, String),
    /// Changes '{0}' write causes error. {1}
    ChangesWrite(String, String),
    /// Fork '{0}' read causes error. {1}
    ForkRead(String, String),
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

// User Carbonado Operations
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
    rgb_wallets: &RgbAccountV1,
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
        Some(RGB_ACCOUNT_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn store_transfers(
    sk: &str,
    name: &str,
    rgb_transfers: &RgbTransfersV1,
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
        Some(RGB_TRANSFER_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))
}

pub async fn store_offers(
    sk: &str,
    name: &str,
    rgb_offers: &RgbOffers,
) -> Result<(), StorageError> {
    let data = to_allocvec(rgb_offers)
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

pub async fn store_bids(sk: &str, name: &str, rgb_bids: &RgbBids) -> Result<(), StorageError> {
    let data = to_allocvec(rgb_bids)
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

pub async fn retrieve_wallets(sk: &str, name: &str) -> Result<RgbAccountV1, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, metadata) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbAccountV1::default())
    } else {
        let mut version: [u8; 8] = default!();
        if let Some(metadata) = metadata {
            version.copy_from_slice(&metadata);
        }

        let rgb_wallets = RgbAccountVersions::from_bytes(data, version)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        Ok(rgb_wallets)
    }
}

pub async fn retrieve_transfers(sk: &str, name: &str) -> Result<RgbTransfersV1, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, metadata) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbTransfersV1::default())
    } else {
        let mut version: [u8; 8] = default!();
        if let Some(metadata) = metadata {
            version.copy_from_slice(&metadata);
        }

        let rgb_wallets = RgbtransferVersions::from_bytes(data, version)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        Ok(rgb_wallets)
    }
}

pub async fn retrieve_offers(sk: &str, name: &str) -> Result<RgbOffers, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbOffers::default())
    } else {
        let rgb_offers = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        Ok(rgb_offers)
    }
}

pub async fn retrieve_bids(sk: &str, name: &str) -> Result<RgbBids, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let (data, _) = retrieve(sk, &format!("{hashed_name}.c15"), vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RgbBids::default())
    } else {
        let rgb_bids = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;
        Ok(rgb_bids)
    }
}

// CDRT Operations
pub async fn cdrt_store_wallets(sk: &str, name: &str, changes: &[u8]) -> Result<(), StorageError> {
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

    store(
        sk,
        main_name,
        &data,
        true,
        Some(RGB_ACCOUNT_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

pub async fn cdrt_retrieve_wallets(sk: &str, name: &str) -> Result<LocalRgbAccount, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (data, metadata) = retrieve(sk, main_name, vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(LocalRgbAccount {
            version: automerge::AutoCommit::new().save(),
            rgb_account: RgbAccountV1::default(),
        })
    } else {
        let mut version: [u8; 8] = default!();
        if let Some(metadata) = metadata {
            version.copy_from_slice(&metadata);
        }

        let mut original_version = automerge::AutoCommit::new();
        let rgb_account = RgbAccountVersions::from_bytes(data, version)
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
            Some(RGB_ACCOUNT_VERSION.to_vec()),
        )
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

        Ok(LocalRgbAccount {
            version: fork_version.save(),
            rgb_account,
        })
    }
}

pub async fn retrieve_public_offers(name: &str) -> Result<LocalRgbOffers, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;
    if data.is_empty() {
        Ok(LocalRgbOffers {
            version: automerge::AutoCommit::new().save(),
            rgb_offers: RgbPublicSwaps::default(),
        })
    } else {
        let mut original_version = automerge::AutoCommit::new();
        let rgb_offers: RgbPublicSwaps = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

        reconcile(&mut original_version, rgb_offers.clone())
            .map_err(|op| StorageError::Reconcile(name.to_string(), op.to_string()))?;

        let mut fork_version = original_version.fork();

        marketplace_store(
            original_name,
            &fork_version.save(),
            Some(RGB_STRICT_TYPE_VERSION.to_vec()),
        )
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

        Ok(LocalRgbOffers {
            version: fork_version.save(),
            rgb_offers,
        })
    }
}

pub async fn retrieve_auctions_offers(
    bundle_id: &str,
    name: &str,
) -> Result<LocalRgbAuctions, StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (data, _) = auctions_retrieve(bundle_id, main_name)
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;
    if data.is_empty() {
        Ok(LocalRgbAuctions {
            version: automerge::AutoCommit::new().save(),
            rgb_offers: RgbAuctionSwaps::default(),
        })
    } else {
        let mut original_version = automerge::AutoCommit::new();
        let rgb_offers: RgbAuctionSwaps = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

        reconcile(&mut original_version, rgb_offers.clone())
            .map_err(|op| StorageError::Reconcile(name.to_string(), op.to_string()))?;

        let mut fork_version = original_version.fork();

        auctions_store(
            bundle_id,
            original_name,
            &fork_version.save(),
            Some(RGB_STRICT_TYPE_VERSION.to_vec()),
        )
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

        Ok(LocalRgbAuctions {
            version: fork_version.save(),
            rgb_offers,
        })
    }
}

pub async fn store_public_offers(name: &str, changes: &[u8]) -> Result<(), StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (original_bytes, _) = marketplace_retrieve(original_name)
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    let mut original_version = automerge::AutoCommit::load(&original_bytes)
        .map_err(|op| StorageError::ForkRead(name.to_string(), op.to_string()))?;

    let mut fork_version = automerge::AutoCommit::load(changes)
        .map_err(|op| StorageError::ChangesRetrieve(name.to_string(), op.to_string()))?;

    original_version
        .merge(&mut fork_version)
        .map_err(|op| StorageError::MergeWrite(name.to_string(), op.to_string()))?;

    let public_offers: RgbPublicSwaps = hydrate(&original_version).unwrap();

    let data = to_allocvec(&public_offers)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    marketplace_store(main_name, &data, Some(RGB_STRICT_TYPE_VERSION.to_vec()))
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

pub async fn store_auction_offers(
    bundle_id: &str,
    name: &str,
    changes: &[u8],
) -> Result<(), StorageError> {
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (original_bytes, _) = marketplace_retrieve(original_name)
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    let mut original_version = automerge::AutoCommit::load(&original_bytes)
        .map_err(|op| StorageError::ForkRead(name.to_string(), op.to_string()))?;

    let mut fork_version = automerge::AutoCommit::load(changes)
        .map_err(|op| StorageError::ChangesRetrieve(name.to_string(), op.to_string()))?;

    original_version
        .merge(&mut fork_version)
        .map_err(|op| StorageError::MergeWrite(name.to_string(), op.to_string()))?;

    let auction_offers: RgbAuctionSwaps = hydrate(&original_version).unwrap();
    let data = to_allocvec(&auction_offers)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    auctions_store(
        bundle_id,
        main_name,
        &data,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

pub async fn retrieve_swap_offer_bid(
    sk: &str,
    name: &str,
    expire_at: Option<i64>,
) -> Result<LocalRgbOfferBid, StorageError> {
    let mut hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    if let Some(expire_at) = expire_at {
        hashed_name = format!("{hashed_name}-{expire_at}");
    }

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (data, _) = retrieve(sk, main_name, vec![])
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;
    if data.is_empty() {
        Ok(LocalRgbOfferBid {
            version: automerge::AutoCommit::new().save(),
            rgb_bid: RgbBidSwap::default(),
        })
    } else {
        let mut original_version = automerge::AutoCommit::new();
        let rgb_offer_bid: RgbBidSwap = from_bytes(&data)
            .map_err(|op| StorageError::StrictRetrieve(name.to_string(), op.to_string()))?;

        reconcile(&mut original_version, rgb_offer_bid.clone())
            .map_err(|op| StorageError::Reconcile(name.to_string(), op.to_string()))?;

        let mut fork_version = original_version.fork();

        store(
            sk,
            original_name,
            &fork_version.save(),
            false,
            Some(RGB_STRICT_TYPE_VERSION.to_vec()),
        )
        .await
        .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

        Ok(LocalRgbOfferBid {
            version: fork_version.save(),
            rgb_bid: rgb_offer_bid,
        })
    }
}

pub async fn store_swap_offer_bid(
    sk: &str,
    name: &str,
    changes: &[u8],
    expire_at: Option<i64>,
) -> Result<(), StorageError> {
    let mut hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_lowercase();

    if let Some(expire_at) = expire_at {
        hashed_name = format!("{hashed_name}-{expire_at}");
    }

    let main_name = &format!("{hashed_name}.c15");
    let original_name = &format!("{hashed_name}-diff.c15");

    let (original_bytes, _) = marketplace_retrieve(original_name)
        .await
        .map_err(|op| StorageError::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    let mut original_version = automerge::AutoCommit::load(&original_bytes)
        .map_err(|op| StorageError::ForkRead(name.to_string(), op.to_string()))?;

    let mut fork_version = automerge::AutoCommit::load(changes)
        .map_err(|op| StorageError::ChangesRetrieve(name.to_string(), op.to_string()))?;

    original_version
        .merge(&mut fork_version)
        .map_err(|op| StorageError::MergeWrite(name.to_string(), op.to_string()))?;

    let rgb_bid: RgbBidSwap = hydrate(&original_version).unwrap();

    let data = to_allocvec(&rgb_bid)
        .map_err(|op| StorageError::StrictWrite(name.to_string(), op.to_string()))?;

    store(
        sk,
        main_name,
        &data,
        false,
        Some(RGB_STRICT_TYPE_VERSION.to_vec()),
    )
    .await
    .map_err(|op| StorageError::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}
