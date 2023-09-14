use autosurgeon::{reconcile, Hydrate, Reconcile};
use bitcoin_scripts::address::AddressCompat;
use garde::Validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{structs::AllocationDetail, validators::RGBContext};

use super::{
    crdt::LocalRgbOffers,
    fs::{retrieve_public_offers, store_public_offers, RgbPersistenceError},
};

pub type AssetId = String;
pub type OfferId = String;
pub type BidId = String;

#[derive(
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
    Deserialize,
    Reconcile,
    Hydrate,
    Clone,
    Debug,
    Display,
    Default,
)]
pub enum RgbOrderStatus {
    #[default]
    #[display(inner)]
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "fill")]
    Fill,
}

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Display)]
#[garde(context(RGBContext))]
#[display("{offer_id} / {contract_id}:{asset_amount} / {bitcoin_price}")]
pub struct RgbOffer {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(skip)]
    pub offer_status: RgbOrderStatus,
    #[garde(ascii)]
    pub contract_id: AssetId,
    #[garde(ascii)]
    pub iface: String,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_price: u64,
    #[garde(ascii)]
    pub seller_psbt: String,
    #[garde(ascii)]
    pub seller_address: String,
}
impl RgbOffer {
    pub(crate) fn new(
        contract_id: String,
        iface: String,
        allocations: Vec<AllocationDetail>,
        seller_address: AddressCompat,
        bitcoin_price: u64,
        psbt: String,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        let asset_amount = allocations
            .clone()
            .into_iter()
            .map(|a| match a.value {
                crate::structs::AllocationValue::Value(amount) => amount,
                crate::structs::AllocationValue::UDA(_) => 1,
            })
            .sum();

        let mut asset_utxos: Vec<String> = allocations.into_iter().map(|a| a.utxo).collect();
        asset_utxos.sort();

        for asset_utxo in asset_utxos {
            hasher.update(asset_utxo.as_bytes());
        }

        RgbOffer {
            offer_id: hasher.finalize().to_hex().to_string(),
            offer_status: RgbOrderStatus::Open,
            contract_id,
            iface,
            asset_amount,
            bitcoin_price,
            seller_psbt: psbt,
            seller_address: seller_address.to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Default, Display)]
#[garde(context(RGBContext))]
#[display("{bid_id} / {contract_id}:{asset_amount} / {bitcoin_amount}")]
pub struct RgbBid {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bid_id: BidId,
    #[garde(skip)]
    pub bid_status: RgbOrderStatus,
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(skip)]
    pub contract_id: AssetId,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_amount: u64,
    #[garde(ascii)]
    pub buyer_psbt: String,
    #[garde(ascii)]
    pub buyer_outpoint: String,
}

impl RgbBid {
    pub(crate) fn new(
        offer_id: OfferId,
        contract_id: AssetId,
        asset_amount: u64,
        bitcoin_price: u64,
        bitcoin_utxos: Vec<String>,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();

        let mut allocations = bitcoin_utxos;
        allocations.sort();

        for allocation in allocations {
            hasher.update(allocation.as_bytes());
        }

        RgbBid {
            bid_id: hasher.finalize().to_string(),
            bid_status: RgbOrderStatus::Open,
            offer_id,
            contract_id,
            asset_amount,
            bitcoin_amount: bitcoin_price,
            ..Default::default()
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Display)]
#[garde(context(RGBContext))]
#[display("{bid_id} / {offer_id}")]
pub struct RgbSwap {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bid_id: BidId,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub fee: u64,
    #[garde(ascii)]
    pub swap_psbt: String,
    #[garde(ascii)]
    pub consignment: String,
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbOffers {
    pub offers: BTreeMap<AssetId, Vec<RgbOffer>>,
    pub bids: BTreeMap<OfferId, Vec<BidId>>,
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbBids {
    pub bids: BTreeMap<AssetId, Vec<RgbBid>>,
}

#[derive(Clone, Eq, PartialEq, Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbOfferErrors {
    IO(RgbPersistenceError),
    NoOffer(String),
    AutoMerge(String),
}

pub async fn get_public_offer(offer_id: OfferId) -> Result<RgbOffer, RgbOfferErrors> {
    let LocalRgbOffers { doc: _, rgb_offers } =
        retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut public_offers = vec![];
    for offers in rgb_offers.offers.values() {
        public_offers.extend(offers);
    }

    let offer = match public_offers.into_iter().find(|x| x.offer_id == offer_id) {
        Some(offer) => offer.clone(),
        _ => return Err(RgbOfferErrors::NoOffer(offer_id)),
    };

    Ok(offer)
}

pub async fn publish_offer(new_offer: RgbOffer) -> Result<(), RgbOfferErrors> {
    let LocalRgbOffers {
        doc,
        mut rgb_offers,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&doc)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;
    if let Some(offers) = rgb_offers.offers.get(&new_offer.contract_id) {
        let mut avaliable_offers = offers.to_owned();
        avaliable_offers.push(new_offer.clone());
        rgb_offers
            .offers
            .insert(new_offer.clone().contract_id, avaliable_offers);
    } else {
        rgb_offers
            .offers
            .insert(new_offer.clone().contract_id, vec![new_offer]);
    }

    // TODO: Add change verification (accept only addition operation)
    reconcile(&mut local_copy, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;
    store_public_offers(local_copy.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn publish_bid(new_bid: RgbBid) -> Result<(), RgbOfferErrors> {
    let _ = get_public_offer(new_bid.clone().offer_id).await?;
    let LocalRgbOffers {
        doc,
        mut rgb_offers,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&doc)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    if let Some(bids) = rgb_offers.bids.get(&new_bid.offer_id) {
        let mut avaliable_bids = bids.to_owned();
        avaliable_bids.push(new_bid.bid_id);
        rgb_offers.bids.insert(new_bid.offer_id, avaliable_bids);
    } else {
        rgb_offers
            .bids
            .insert(new_bid.offer_id, vec![new_bid.bid_id]);
    }

    // TODO: Add change verification (accept only addition operation)
    reconcile(&mut local_copy, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;
    store_public_offers(local_copy.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}
