#![allow(deprecated)]
use amplify::{
    confinement::{Confined, U32},
    hex::{FromHex, ToHex},
    Array, ByteArray, Bytes32,
};
use autosurgeon::{reconcile, Hydrate, Reconcile};
use baid58::{Baid58ParseError, FromBaid58, ToBaid58};
use bitcoin::psbt::Psbt as PsbtV0;
use bitcoin_30::secp256k1::{ecdh::SharedSecret, PublicKey, Secp256k1, SecretKey};
use bitcoin_scripts::address::AddressCompat;
use bp::Txid;
use core::fmt::Display;
use garde::Validate;

use rgbstd::{
    containers::{Bindle, Transfer},
    validation::{AnchoredBundle, ConsignmentApi},
};
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    collections::{btree_map, BTreeMap},
    fmt::{self, Formatter},
    str::FromStr,
};
use strict_encoding::{
    StrictDecode, StrictDeserialize, StrictDumb, StrictEncode, StrictSerialize, StrictType,
};

use crate::{
    rgb::{
        constants::LIB_NAME_BITMASK,
        crdt::{LocalRgbAuctions, LocalRgbOfferBid, LocalRgbOffers},
        fs::{
            retrieve_auctions_offers, retrieve_public_offers, retrieve_swap_offer_bid,
            store_auction_offers, store_public_offers, store_swap_bids, RgbPersistenceError,
        },
    },
    structs::PsbtFeeRequest,
};
use crate::{structs::AllocationDetail, validators::RGBContext};

type AssetId = String;
type OfferId = String;
type BidId = String;
type TransferId = String;

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

#[derive(
    Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Default, Reconcile, Hydrate, Display,
)]
#[serde(rename_all = "camelCase")]
#[display(inner)]
pub enum RgbSwapStrategy {
    #[default]
    #[serde(rename = "auction")]
    Auction,
    #[serde(rename = "p2p")]
    P2P,
    #[serde(rename = "hotswap")]
    HotSwap,
    #[serde(rename = "airdrop")]
    Airdrop,
}

#[derive(
    Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Default, Reconcile, Hydrate, Display,
)]
#[serde(rename_all = "camelCase")]
#[display(inner)]
pub enum RgbAuctionStrategy {
    #[default]
    #[serde(rename = "auction")]
    Auction,
    #[serde(rename = "airdrop")]
    Airdrop { max_claim: String },
}

#[derive(Clone, Debug, Display, Default, Error)]
#[display(doc_comments)]
pub struct RgbOfferOptions {
    pub bundle_id: Option<String>,
    pub max_claim: Option<u64>,
    pub fee_airdrop: Option<PsbtFeeRequest>,
}

impl RgbOfferOptions {
    pub fn new(secret: String) -> Self {
        let secp = Secp256k1::new();
        let secret = hex::decode(secret).expect("cannot decode hex sk in new RgbOffer");
        let secret_key = SecretKey::from_slice(&secret).expect("error parsing sk in new RgbOffer");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let bundle_id = Some(public_key.to_hex());
        Self {
            bundle_id,
            fee_airdrop: None,
            max_claim: None,
        }
    }

    pub fn new_airdrop(secret: String, fee: PsbtFeeRequest, max: u64) -> Self {
        let secp = Secp256k1::new();
        let secret = hex::decode(secret).expect("cannot decode hex sk in new RgbOffer");
        let secret_key = SecretKey::from_slice(&secret).expect("error parsing sk in new RgbOffer");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let bundle_id = Some(public_key.to_hex());
        Self {
            bundle_id,
            fee_airdrop: Some(fee),
            max_claim: Some(max),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Display, Default)]
#[garde(context(RGBContext))]
#[display("{offer_id} / {contract_id}:{asset_amount} / {bitcoin_price}")]
pub struct RgbOffer {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(skip)]
    pub status: RgbOrderStatus,
    #[garde(ascii)]
    pub contract_id: AssetId,
    #[garde(ascii)]
    pub iface: String,
    #[garde(skip)]
    pub strategy: RgbSwapStrategy,
    #[garde(ascii)]
    pub pub_key: String,
    #[garde(ascii)]
    pub terminal: String,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub asset_precision: u8,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_price: u64,
    #[garde(ascii)]
    pub seller_address: String,
    #[garde(ascii)]
    pub seller_psbt: String,
    #[garde(skip)]
    pub expire_at: Option<i64>,
    #[garde(skip)]
    pub bundle_id: Option<String>,
    #[garde(skip)]
    pub max_claim: Option<u64>,
    #[garde(skip)]
    pub transfer_id: Option<String>,
}

impl RgbOffer {
    pub fn is_fill(self) -> bool {
        self.status == RgbOrderStatus::Fill
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        secret: String,
        contract_id: String,
        iface: String,
        allocations: Vec<AllocationDetail>,
        asset_amount: u64,
        asset_precision: u8,
        seller_address: AddressCompat,
        bitcoin_price: u64,
        psbt: String,
        terminal: String,
        strategy: RgbSwapStrategy,
        expire_at: Option<i64>,
        bundle_id: Option<String>,
        max_claim: Option<u64>,
    ) -> Self {
        let secp = Secp256k1::new();
        let secret = hex::decode(secret).expect("cannot decode hex sk in new RgbOffer");
        let secret_key = SecretKey::from_slice(&secret).expect("error parsing sk in new RgbOffer");
        let pub_key = PublicKey::from_secret_key(&secp, &secret_key).to_hex();

        let mut asset_utxos: Vec<String> = allocations.into_iter().map(|a| a.utxo).collect();
        asset_utxos.sort();

        let mut hasher = blake3::Hasher::new();
        hasher.update(contract_id.as_bytes());
        for asset_utxo in asset_utxos {
            hasher.update(asset_utxo.as_bytes());
        }

        let id = Array::from_array(hasher.finalize().into());
        let order_id = OrderId(id);
        let order_id = order_id.to_baid58_string();

        RgbOffer {
            offer_id: order_id.to_string(),
            status: RgbOrderStatus::Open,
            contract_id,
            iface,
            asset_amount,
            asset_precision,
            bitcoin_price,
            seller_psbt: psbt,
            seller_address: seller_address.to_string(),
            pub_key,
            expire_at,
            terminal,
            strategy,
            bundle_id,
            max_claim,
            ..Default::default()
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Display, Default)]
#[garde(context(RGBContext))]
#[display("{offer_id} / {contract_id}:{asset_amount} / {bitcoin_price}")]
pub struct RgbOfferSwap {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(ascii)]
    pub contract_id: AssetId,
    #[garde(ascii)]
    pub iface: String,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub asset_precision: u8,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_price: u64,
    #[garde(ascii)]
    pub seller_address: String,
    #[garde(skip)]
    pub strategy: RgbSwapStrategy,
    #[garde(ascii)]
    pub pub_key: String,
    #[garde(ascii)]
    pub seller_psbt: Option<String>,
    #[garde(skip)]
    pub bundle_id: Option<String>,
    #[garde(skip)]
    pub max_claim: Option<u64>,
    #[garde(skip)]
    pub expire_at: Option<i64>,
}

impl From<RgbOffer> for RgbOfferSwap {
    fn from(value: RgbOffer) -> Self {
        let RgbOffer {
            offer_id,
            contract_id,
            iface,
            asset_amount,
            bitcoin_price,
            seller_psbt,
            seller_address,
            pub_key,
            asset_precision,
            strategy,
            expire_at,
            bundle_id,
            max_claim,
            ..
        } = value;

        let seller_psbt = if seller_psbt.is_empty() {
            None
        } else {
            Some(seller_psbt)
        };

        Self {
            offer_id,
            strategy,
            contract_id,
            iface,
            asset_amount,
            asset_precision,
            bitcoin_price,
            seller_psbt,
            seller_address,
            pub_key,
            bundle_id,
            expire_at,
            max_claim,
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
    pub status: RgbOrderStatus,
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(ascii)]
    pub pub_key: String,
    #[garde(skip)]
    pub contract_id: AssetId,
    #[garde(skip)]
    pub iface: String,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub asset_precision: u8,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_amount: u64,
    #[garde(ascii)]
    pub buyer_invoice: String,
    #[garde(ascii)]
    pub buyer_psbt: Option<String>,
    #[garde(skip)]
    pub transfer_id: Option<String>,
    #[garde(skip)]
    pub transfer: Option<String>,
    #[garde(skip)]
    pub swap_psbt: Option<String>,
}

impl RgbBid {
    pub fn is_fill(self) -> bool {
        self.status == RgbOrderStatus::Fill
    }

    pub(crate) fn new(
        secret: String,
        offer_id: OfferId,
        contract_id: AssetId,
        asset_amount: u64,
        asset_precision: u8,
        bitcoin_price: u64,
        bitcoin_utxos: Vec<String>,
    ) -> Self {
        let secp = Secp256k1::new();
        let secret = hex::decode(secret).expect("cannot decode hex sk in new RgbBid");
        let secret_key = SecretKey::from_slice(&secret).expect("error parsing sk in new RgbBid");
        let pub_key = PublicKey::from_secret_key(&secp, &secret_key).to_hex();

        let mut allocations = bitcoin_utxos;
        allocations.sort();

        let mut hasher = blake3::Hasher::new();
        hasher.update(contract_id.as_bytes());
        for allocation in allocations {
            hasher.update(allocation.as_bytes());
        }

        let id = Array::from_array(hasher.finalize().into());
        let order_id = OrderId(id);
        let order_id = order_id.to_baid58_string();

        RgbBid {
            bid_id: order_id.to_string(),
            status: RgbOrderStatus::Open,
            offer_id,
            contract_id,
            asset_amount,
            asset_precision,
            bitcoin_amount: bitcoin_price,
            pub_key,
            ..Default::default()
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Default, Display)]
#[garde(context(RGBContext))]
#[display("{bid_id} / {contract_id}:{asset_amount} / {bitcoin_amount}")]
pub struct RgbBidSwap {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bid_id: BidId,
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: OfferId,
    #[garde(ascii)]
    pub pub_key: String,
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub iface: String,
    #[garde(skip)]
    pub contract_id: AssetId,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub asset_precision: u8,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_amount: u64,
    #[garde(ascii)]
    pub buyer_invoice: String,
    #[garde(skip)]
    pub buyer_psbt: Option<String>,
    #[garde(skip)]
    pub transfer_id: Option<String>,
    #[garde(skip)]
    pub transfer: Option<String>,
    #[garde(skip)]
    pub swap_psbt: Option<String>,
    #[garde(skip)]
    pub swap_outpoint: Option<String>,
    #[garde(skip)]
    pub swap_amount: Option<u64>,
    #[garde(skip)]
    pub swap_commit: Option<String>,
}

impl From<RgbBid> for RgbBidSwap {
    fn from(value: RgbBid) -> Self {
        let RgbBid {
            bid_id,
            offer_id,
            contract_id,
            asset_amount,
            asset_precision,
            bitcoin_amount,
            buyer_psbt,
            buyer_invoice,
            pub_key,
            transfer_id,
            transfer,
            iface,
            swap_psbt,
            ..
        } = value;

        Self {
            bid_id,
            offer_id,
            contract_id,
            iface,
            asset_amount,
            asset_precision,
            bitcoin_amount,
            buyer_psbt,
            buyer_invoice,
            pub_key,
            transfer_id,
            transfer,
            swap_psbt,
            ..Default::default()
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Default, Display)]
#[garde(context(RGBContext))]
#[display("{bid_id}:{asset_amount} = {bitcoin_amount}")]
pub struct PublicRgbBid {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bid_id: BidId,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_amount: u64,
    #[garde(ascii)]
    pub pub_key: String,
}

impl From<RgbBidSwap> for PublicRgbBid {
    fn from(value: RgbBidSwap) -> Self {
        let RgbBidSwap {
            bid_id,
            asset_amount,
            bitcoin_amount,
            pub_key,
            ..
        } = value;

        Self {
            bid_id,
            asset_amount,
            bitcoin_amount,
            pub_key,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbOffers {
    pub offers: BTreeMap<AssetId, Vec<RgbOffer>>,
    pub bids: BTreeMap<OfferId, BTreeMap<BidId, RgbBid>>,
}

impl RgbOffers {
    pub fn get_offers(self, bundle_id: String) -> Vec<RgbOffer> {
        let mut item = vec![];
        for offers in self.offers.values() {
            let offers = offers.to_vec();

            item.extend(
                offers
                    .into_iter()
                    .filter(|x| x.bundle_id.clone().unwrap_or_default() == bundle_id),
            );
        }
        item
    }

    pub fn get_offer(self, offer_id: OfferId) -> Option<RgbOffer> {
        let mut item = None;
        for offers in self.offers.values() {
            if let Some(offer) = offers.iter().find(|x| x.offer_id == offer_id) {
                item = Some(offer.to_owned());
                break;
            }
        }
        item
    }

    pub fn save_offer(mut self, contract_id: AssetId, offer: RgbOffer) -> Self {
        if let Some(offers) = self.offers.get(&contract_id) {
            let mut available_offers = offers.to_owned();
            if let Some(position) = available_offers
                .iter()
                .position(|x| x.offer_id == offer.offer_id)
            {
                available_offers.remove(position);
                available_offers.insert(position, offer.clone());
            } else {
                available_offers.push(offer.clone());
            }

            available_offers.push(offer.clone());
            self.offers.insert(contract_id, available_offers);
        } else {
            self.offers.insert(contract_id, vec![offer.clone()]);
        }
        self
    }
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbBids {
    pub bids: BTreeMap<AssetId, Vec<RgbBid>>,
}

impl RgbBids {
    pub fn save_bid(mut self, contract_id: AssetId, bid: RgbBid) -> Self {
        if let Some(offers) = self.bids.get(&contract_id) {
            let mut available_bids: Vec<RgbBid> = offers.to_owned();
            if let Some(position) = available_bids.iter().position(|x| x.bid_id == bid.bid_id) {
                available_bids.remove(position);
                available_bids.insert(position, bid.clone());
            } else {
                available_bids.push(bid.clone());
            }

            available_bids.push(bid.clone());
            self.bids.insert(contract_id, available_bids);
        } else {
            self.bids.insert(contract_id, vec![bid.clone()]);
        }
        self
    }
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbPublicSwaps {
    pub offers: BTreeMap<AssetId, Vec<RgbOfferSwap>>,
    pub bids: BTreeMap<OfferId, BTreeMap<BidId, PublicRgbBid>>,
}

impl RgbPublicSwaps {
    pub fn get_offer(self, offer_id: OfferId) -> Option<RgbOfferSwap> {
        let mut public_offers = vec![];
        for offers in self.offers.values() {
            public_offers.extend(offers);
        }

        public_offers
            .into_iter()
            .find(|x| x.offer_id == offer_id)
            .cloned()
    }

    pub fn save_offer(mut self, contract_id: AssetId, offer: RgbOfferSwap) -> Self {
        if let Some(offers) = self.offers.get(&contract_id) {
            let mut available_offers = offers.to_owned();
            if let Some(position) = available_offers
                .iter()
                .position(|x| x.offer_id == offer.offer_id)
            {
                available_offers.remove(position);
                available_offers.insert(position, offer.clone());
            } else {
                available_offers.push(offer.clone());
            }

            available_offers.push(offer.clone());
            self.offers.insert(contract_id, available_offers);
        } else {
            self.offers.insert(contract_id, vec![offer.clone()]);
        }
        self
    }

    pub fn save_bid(mut self, offer_id: OfferId, bid: RgbBidSwap) -> Self {
        let new_public_bid = PublicRgbBid::from(bid);
        let PublicRgbBid { bid_id, .. } = new_public_bid.clone();
        if let Some(bids) = self.bids.get(&offer_id) {
            let mut available_bids = bids.to_owned();
            available_bids.insert(bid_id, new_public_bid);
            self.bids.insert(offer_id.clone(), available_bids);
        } else {
            self.bids
                .insert(offer_id.clone(), bmap! { bid_id => new_public_bid });
        }
        self
    }
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbAuctionSwaps {
    pub bundle_id: String,
    pub strategy: RgbAuctionStrategy,
    pub items: Vec<RgbOfferSwap>,
    pub bids: BTreeMap<OfferId, Vec<RgbBidSwap>>,
}

impl RgbAuctionSwaps {
    pub fn get_offer(self, bundle_id: String, offer_id: String) -> Option<RgbOfferSwap> {
        self.items.into_iter().find(|x| {
            x.bundle_id.clone().unwrap_or_default() == bundle_id && x.offer_id == offer_id
        })
    }

    pub fn get_offers(self, bundle_id: String) -> Vec<RgbOfferSwap> {
        self.items
            .into_iter()
            .filter(|x| x.bundle_id.clone().unwrap_or_default() == bundle_id)
            .collect()
    }

    pub fn save_bid(mut self, offer_id: OfferId, bid: RgbBidSwap) -> Self {
        let available_bids = if let Some(bids) = self.bids.get(&offer_id) {
            let mut available_bids = bids.to_owned();
            available_bids.push(bid);
            available_bids
        } else {
            vec![bid]
        };

        self.bids.insert(offer_id.clone(), available_bids);
        self
    }

    pub fn save_offers(mut self, offers: Vec<RgbOfferSwap>) -> Self {
        for offer in offers.into_iter() {
            self = self.save_offer(offer);
        }
        self
    }

    fn save_offer(mut self, offer: RgbOfferSwap) -> Self {
        let mut available_offers = self.items.clone();
        if let Some(position) = available_offers
            .iter()
            .position(|x| x.offer_id == offer.offer_id)
        {
            available_offers.remove(position);
            available_offers.insert(position, offer.clone());
        } else {
            self.bundle_id = offer.bundle_id.clone().unwrap_or_default();
            available_offers.push(offer.clone());
        }
        self.items = available_offers;
        self
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbOfferErrors {
    /// Occurs an error in retrieve offers. {0}
    IO(RgbPersistenceError),
    /// Occurs an error in retrieve keys. {0}
    Keys(String),
    /// Offer #{0} is not found in public orderbook.
    NoOffer(String),
    /// Bid #{0} is not found in public orderbook.
    NoBid(String),
    /// Collection offers empty
    NoBundle,
    /// Occurs an error in merge step. {0}
    AutoMerge(String),
}

pub async fn get_public_offers() -> Result<Vec<RgbOfferSwap>, RgbOfferErrors> {
    let LocalRgbOffers { rgb_offers, .. } =
        retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut public_offers = vec![];
    for offers in rgb_offers.offers.values() {
        public_offers.extend(offers.iter().cloned());
    }
    Ok(public_offers)
}

pub async fn get_public_offer(offer_id: OfferId) -> Result<RgbOfferSwap, RgbOfferErrors> {
    let LocalRgbOffers { rgb_offers, .. } =
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

pub async fn get_auction(bundle_id: &str) -> Result<Option<RgbAuctionSwaps>, RgbOfferErrors> {
    let file_name = format!("bundle:{bundle_id}");

    let LocalRgbAuctions { rgb_offers, .. } = retrieve_auctions_offers(bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(Some(rgb_offers))
}

pub async fn get_auction_offer(
    bundle_id: &str,
    offer_id: OfferId,
) -> Result<Option<RgbOfferSwap>, RgbOfferErrors> {
    let file_name = format!("bundle:{bundle_id}");

    let LocalRgbAuctions { rgb_offers, .. } = retrieve_auctions_offers(bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(rgb_offers.get_offer(bundle_id.to_owned(), offer_id.clone()))
}

pub async fn get_public_bid(
    offer_id: OfferId,
    bid_id: BidId,
) -> Result<PublicRgbBid, RgbOfferErrors> {
    let LocalRgbOffers { rgb_offers, .. } =
        retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let public_bids = match rgb_offers.bids.get(&offer_id) {
        Some(bids) => bids,
        _ => return Err(RgbOfferErrors::NoOffer(offer_id)),
    };

    let public_bid = match public_bids.get(&bid_id) {
        Some(bid) => bid.clone(),
        _ => return Err(RgbOfferErrors::NoBid(bid_id)),
    };

    Ok(public_bid)
}

pub async fn get_swap_bids_by_offer(
    sk: &str,
    offer: RgbOffer,
) -> Result<Vec<RgbBidSwap>, RgbOfferErrors> {
    let RgbOffer {
        offer_id,
        expire_at,
        ..
    } = offer;

    let LocalRgbOffers { rgb_offers, .. } =
        retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let public_bids: Vec<PublicRgbBid> = match rgb_offers.bids.get(&offer_id) {
        Some(bids) => bids.values().cloned().collect(),
        _ => return Err(RgbOfferErrors::NoOffer(offer_id)),
    };

    let mut swap_bids = vec![];
    for bid in public_bids {
        let PublicRgbBid {
            bid_id,
            pub_key: public,
            ..
        } = bid.clone();
        let secret = hex::decode(sk).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
        let secret_key =
            SecretKey::from_slice(&secret).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
        let public_key =
            PublicKey::from_str(&public).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;

        let share_sk = SharedSecret::new(&public_key, &secret_key);
        let share_sk = share_sk.display_secret().to_string();

        let file_name = format!("{offer_id}-{bid_id}");
        match retrieve_swap_offer_bid(&share_sk, &file_name, expire_at).await {
            Ok(local_copy) => swap_bids.push(local_copy.rgb_bid),
            _ => continue,
        }
    }

    Ok(swap_bids)
}

pub async fn get_swap_bid_by_seller(
    sk: &str,
    offer_id: String,
    bid_id: BidId,
    expire_at: Option<i64>,
) -> Result<RgbBidSwap, RgbOfferErrors> {
    let bid = get_public_bid(offer_id.clone(), bid_id.clone()).await?;

    let secret = hex::decode(sk).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
    let secret_key =
        SecretKey::from_slice(&secret).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
    let public_key =
        PublicKey::from_str(&bid.pub_key).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;

    let share_sk = SharedSecret::new(&public_key, &secret_key);
    let share_sk = share_sk.display_secret().to_string();

    let file_name = format!("{offer_id}-{bid_id}");
    let LocalRgbOfferBid { rgb_bid, .. } =
        retrieve_swap_offer_bid(&share_sk, &file_name, expire_at)
            .await
            .map_err(RgbOfferErrors::IO)?;

    Ok(rgb_bid)
}

pub async fn get_swap_bid_by_buyer(
    sk: &str,
    offer_id: String,
    bid_id: BidId,
) -> Result<RgbBidSwap, RgbOfferErrors> {
    let RgbOfferSwap {
        expire_at,
        pub_key: public,
        ..
    } = get_public_offer(offer_id.clone()).await?;

    let secret = hex::decode(sk).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
    let secret_key =
        SecretKey::from_slice(&secret).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
    let public_key =
        PublicKey::from_str(&public).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;

    let share_sk = SharedSecret::new(&public_key, &secret_key);
    let share_sk = share_sk.display_secret().to_string();

    let file_name = format!("{offer_id}-{bid_id}");
    let LocalRgbOfferBid { rgb_bid, .. } =
        retrieve_swap_offer_bid(&share_sk, &file_name, expire_at)
            .await
            .map_err(RgbOfferErrors::IO)?;

    Ok(rgb_bid)
}

pub async fn get_auction_highest_bids(
    bundle_id: String,
) -> Result<BTreeMap<OfferId, Vec<RgbBidSwap>>, RgbOfferErrors> {
    let file_name = format!("bundle:{bundle_id}");
    let LocalRgbAuctions { rgb_offers, .. } = retrieve_auctions_offers(&bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    let mut highest_bids = bmap! {};
    for RgbOfferSwap { offer_id, .. } in rgb_offers.clone().get_offers(bundle_id) {
        if let Some(bids) = rgb_offers.bids.get(&offer_id).cloned() {
            if let Some(bid) = bids.iter().max_by_key(|x| x.bitcoin_amount).cloned() {
                highest_bids.insert(offer_id, vec![bid]);
            }
        };
    }

    Ok(highest_bids)
}

pub async fn get_auction_fifo_bids(
    bundle_id: String,
) -> Result<BTreeMap<OfferId, Vec<RgbBidSwap>>, RgbOfferErrors> {
    let file_name = format!("bundle:{bundle_id}");
    let LocalRgbAuctions { rgb_offers, .. } = retrieve_auctions_offers(&bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    let mut first_bids = bmap! {};
    for RgbOfferSwap { offer_id, .. } in rgb_offers.clone().get_offers(bundle_id) {
        if let Some(bids) = rgb_offers.bids.get(&offer_id).cloned() {
            first_bids.insert(offer_id, bids);
        };
    }

    Ok(first_bids)
}

pub async fn get_auction_highest_bid(
    bundle_id: String,
    offer_id: OfferId,
) -> Result<Option<RgbBidSwap>, RgbOfferErrors> {
    let file_name = format!("bundle:{bundle_id}");

    let LocalRgbAuctions { rgb_offers, .. } = retrieve_auctions_offers(&bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    if rgb_offers
        .clone()
        .get_offer(bundle_id.to_owned(), offer_id.clone())
        .is_none()
    {
        return Err(RgbOfferErrors::NoOffer(offer_id.clone()));
    }

    let highest_bid = if let Some(bids) = rgb_offers.bids.get(&offer_id) {
        bids.iter().max_by_key(|x| x.bitcoin_amount).cloned()
    } else {
        None
    };

    Ok(highest_bid)
}

pub async fn publish_public_offer(new_offer: RgbOfferSwap) -> Result<(), RgbOfferErrors> {
    let LocalRgbOffers {
        mut rgb_offers,
        version,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut current_version = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    let contract_id = new_offer.contract_id.clone();
    rgb_offers = rgb_offers.save_offer(contract_id, new_offer);

    reconcile(&mut current_version, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_public_offers(current_version.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn publish_public_offers(new_offers: Vec<RgbOfferSwap>) -> Result<(), RgbOfferErrors> {
    let LocalRgbOffers {
        mut rgb_offers,
        version,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut current_version = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    for new_offer in new_offers {
        let contract_id = new_offer.contract_id.clone();
        rgb_offers = rgb_offers.save_offer(contract_id, new_offer);
    }

    reconcile(&mut current_version, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_public_offers(current_version.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn publish_public_bid(new_bid: RgbBidSwap) -> Result<(), RgbOfferErrors> {
    let RgbBidSwap { offer_id, .. } = new_bid.clone();

    let _ = get_public_offer(offer_id.clone()).await?;
    let LocalRgbOffers {
        mut rgb_offers,
        version,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    rgb_offers = rgb_offers.save_bid(offer_id, new_bid);
    reconcile(&mut local_copy, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_public_offers(local_copy.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn publish_swap_bid(
    sk: &str,
    offer_pub: &str,
    new_bid: RgbBidSwap,
    expire_at: Option<i64>,
) -> Result<(), RgbOfferErrors> {
    let RgbBidSwap {
        bid_id, offer_id, ..
    } = new_bid.clone();

    let secret = hex::decode(sk).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
    let secret_key =
        SecretKey::from_slice(&secret).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;
    let public_key =
        PublicKey::from_str(offer_pub).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;

    let share_sk = SharedSecret::new(&public_key, &secret_key);
    let share_sk = share_sk.display_secret().to_string();

    let file_name = format!("{offer_id}-{bid_id}");

    let LocalRgbOfferBid { version, .. } =
        retrieve_swap_offer_bid(&share_sk, &file_name, expire_at)
            .await
            .map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    reconcile(&mut local_copy, new_bid).map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_swap_bids(&share_sk, &file_name, local_copy.save(), expire_at)
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn publish_auction_offers(
    strategy: RgbAuctionStrategy,
    new_offers: Vec<RgbOfferSwap>,
) -> Result<(), RgbOfferErrors> {
    let RgbOfferSwap { bundle_id, .. } = new_offers[0].clone();
    let bundle_id = bundle_id.unwrap_or_default();
    let file_name = format!("bundle:{bundle_id}");

    let LocalRgbAuctions {
        mut rgb_offers,
        version,
    } = retrieve_auctions_offers(&bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    let mut current_version = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    rgb_offers = rgb_offers.save_offers(new_offers.clone());
    rgb_offers.strategy = strategy;

    reconcile(&mut current_version, rgb_offers.clone())
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_auction_offers(&bundle_id, &file_name, current_version.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    publish_public_offers(new_offers).await?;
    Ok(())
}

pub async fn publish_auction_bid(new_bid: RgbBidSwap) -> Result<(), RgbOfferErrors> {
    let RgbBidSwap { offer_id, .. } = new_bid.clone();
    let RgbOfferSwap { bundle_id, .. } = get_public_offer(offer_id.clone()).await?;
    let bundle_id = bundle_id.unwrap_or_default();
    let file_name = format!("bundle:{bundle_id}");

    let LocalRgbAuctions {
        mut rgb_offers,
        version,
    } = retrieve_auctions_offers(&bundle_id, &file_name)
        .await
        .map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    rgb_offers = rgb_offers.save_bid(offer_id.clone(), new_bid.clone());
    reconcile(&mut local_copy, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_auction_offers(&bundle_id, &file_name, local_copy.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn remove_public_offers(offers: Vec<RgbOffer>) -> Result<(), RgbOfferErrors> {
    let LocalRgbOffers {
        mut rgb_offers,
        version,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&version)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    let current_public_offers = rgb_offers.clone();
    for offer in offers {
        if let Some(public_offers) = current_public_offers.offers.get(&offer.contract_id) {
            let public_offers = public_offers.to_owned();
            if public_offers.iter().any(|x| x.offer_id == offer.offer_id) {
                let others = public_offers
                    .iter()
                    .filter(|x| x.offer_id != offer.offer_id)
                    .map(|x| x.to_owned())
                    .collect();
                rgb_offers.offers.insert(offer.contract_id, others);
                rgb_offers.bids.remove(&offer.offer_id);
            }
        }
    }

    // TODO: Add change verification (accept only addition operation)
    reconcile(&mut local_copy, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_public_offers(local_copy.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn update_transfer_offer(
    offer_id: OfferId,
    consig_id: TransferId,
    rgb_offers: &mut RgbOffers,
) -> Result<(), RgbOfferErrors> {
    let offers = rgb_offers.offers.clone();
    for (contract_id, mut my_offers) in offers {
        if let Some(position) = my_offers.iter().position(|x| x.offer_id == offer_id) {
            let mut offer = my_offers.swap_remove(position);
            offer.transfer_id = Some(consig_id.to_owned());

            my_offers.insert(position, offer);
            rgb_offers.offers.insert(contract_id, my_offers);
            break;
        }
    }
    Ok(())
}

pub async fn update_transfer_bid(
    bid_id: BidId,
    consig_id: TransferId,
    rgb_bids: &mut RgbBids,
) -> Result<(), RgbOfferErrors> {
    let bids = rgb_bids.bids.clone();
    for (contract_id, mut my_bids) in bids {
        if let Some(position) = my_bids.iter().position(|x| x.bid_id == bid_id) {
            let mut offer = my_bids.swap_remove(position);
            offer.transfer_id = Some(consig_id.to_owned());

            my_bids.insert(position, offer);
            rgb_bids.bids.insert(contract_id, my_bids);
            break;
        }
    }
    Ok(())
}

pub async fn complete_offer(
    transfer_id: TransferId,
    rgb_offers: &mut RgbOffers,
) -> Result<Option<RgbOffer>, RgbOfferErrors> {
    let mut offer_filled = None;
    let offers = rgb_offers.offers.clone();
    for (contract_id, mut my_offers) in offers {
        if let Some(position) = my_offers
            .clone()
            .into_iter()
            .position(|x| x.transfer_id.unwrap_or_default() == transfer_id)
        {
            let mut offer = my_offers.swap_remove(position);
            offer.status = RgbOrderStatus::Fill;

            offer_filled = Some(offer.clone());
            my_offers.insert(position, offer);
            rgb_offers.offers.insert(contract_id, my_offers);

            break;
        }
    }
    Ok(offer_filled)
}

pub async fn complete_bid(
    transfer_id: TransferId,
    rgb_bids: &mut RgbBids,
) -> Result<Option<RgbBid>, RgbOfferErrors> {
    let mut bid_filled = None;
    let bids = rgb_bids.bids.clone();
    for (contract_id, mut my_bids) in bids {
        if let Some(position) = my_bids
            .clone()
            .into_iter()
            .position(|x| x.transfer_id.unwrap_or_default() == transfer_id)
        {
            let mut bid = my_bids.swap_remove(position);
            bid.status = RgbOrderStatus::Fill;

            bid_filled = Some(bid.clone());
            my_bids.insert(position, bid);
            rgb_bids.bids.insert(contract_id, my_bids);

            break;
        }
    }
    Ok(bid_filled)
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum PsbtSwapExError {
    /// The Input PSBT is invalid (Unexpected behavior).
    Inconclusive,
}

pub trait PsbtSwapEx<T> {
    type Error: std::error::Error;

    /// Join this [`PartiallySignedTransaction`] with `other` PSBT as described by BIP 174.
    ///
    /// The join method emulate the same behavior of the rpc method `joinpsbts`
    /// See: https://developer.bitcoin.org/reference/rpc/joinpsbts.html
    fn join(self, other: T) -> Result<T, Self::Error>;
}

impl PsbtSwapEx<PsbtV0> for PsbtV0 {
    type Error = PsbtSwapExError;

    fn join(self, other: PsbtV0) -> Result<PsbtV0, Self::Error> {
        // BIP 174: The Combiner must remove any duplicate key-value pairs, in accordance with
        //          the specification. It can pick arbitrarily when conflicts occur.

        // Keeping the highest version
        let mut new_psbt = PsbtV0::from(self).clone();
        // let mut other = other;
        new_psbt.version = cmp::max(new_psbt.version, other.version);

        // Merging xpubs
        for (xpub, (fingerprint1, derivation1)) in other.xpub {
            match new_psbt.xpub.entry(xpub) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert((fingerprint1, derivation1));
                }
                btree_map::Entry::Occupied(mut entry) => {
                    // Here in case of the conflict we select the version with algorithm:
                    // 1) if everything is equal we do nothing
                    // 2) report an error if
                    //    - derivation paths are equal and fingerprints are not
                    //    - derivation paths are of the same length, but not equal
                    //    - derivation paths has different length, but the shorter one
                    //      is not the strict suffix of the longer one
                    // 3) choose longest derivation otherwise

                    let (fingerprint2, derivation2) = entry.get().clone();

                    if (derivation1 == derivation2 && fingerprint1 == fingerprint2)
                        || (derivation1.len() < derivation2.len()
                            && derivation1[..]
                                == derivation2[derivation2.len() - derivation1.len()..])
                    {
                        continue;
                    } else if derivation2[..]
                        == derivation1[derivation1.len() - derivation2.len()..]
                    {
                        entry.insert((fingerprint1, derivation1));
                        continue;
                    }
                    return Err(PsbtSwapExError::Inconclusive);
                }
            }
        }

        new_psbt.proprietary.extend(other.proprietary);
        new_psbt.unknown.extend(other.unknown);

        // new_psbt.inputs.extend(other.inputs);
        let current_inputs = new_psbt.inputs.clone();
        let new_inputs = other.inputs.clone();
        new_inputs.into_iter().for_each(|vin| {
            if !current_inputs.clone().into_iter().any(|x| {
                x.bip32_derivation == vin.bip32_derivation
                    && x.non_witness_utxo.eq(&vin.non_witness_utxo)
            }) {
                new_psbt.inputs.push(vin);
            }
        });

        let current_outputs = new_psbt.outputs.clone();
        let new_outputs = other.outputs.clone();
        new_outputs.into_iter().for_each(|out| {
            if !current_outputs
                .clone()
                .into_iter()
                .any(|x| x.bip32_derivation == out.bip32_derivation)
            {
                new_psbt.outputs.push(out);
            }
        });

        // Transaction
        new_psbt.unsigned_tx.version =
            cmp::max(new_psbt.unsigned_tx.version, other.unsigned_tx.version);

        new_psbt.unsigned_tx.lock_time =
            cmp::max(new_psbt.unsigned_tx.lock_time, other.unsigned_tx.lock_time);

        // new_psbt.unsigned_tx.input.extend(other.unsigned_tx.input);
        let current_inputs = new_psbt.unsigned_tx.input.clone();
        let new_inputs = other.unsigned_tx.input.clone();
        new_inputs.into_iter().for_each(|vin| {
            if !current_inputs
                .clone()
                .into_iter()
                .any(|x| x.previous_output.eq(&vin.previous_output))
            {
                new_psbt.unsigned_tx.input.push(vin);
            }
        });

        let current_outputs = new_psbt.unsigned_tx.output.clone();
        let new_outputs = other.unsigned_tx.output.clone();
        new_outputs.into_iter().for_each(|out| {
            if !current_outputs
                .clone()
                .into_iter()
                .any(|x| x.script_pubkey == out.script_pubkey && x.value == out.value)
            {
                new_psbt.unsigned_tx.output.push(out);
            }
        });

        Ok(new_psbt.clone())
    }
}

/// Swap Order identifier.
///
/// Interface identifier commits to all of the interface data.
#[derive(
    Wrapper,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    Debug,
    From,
    StrictType,
    StrictDumb,
    StrictEncode,
    StrictDecode,
)]
#[wrapper(Deref, BorrowSlice, Hex, Index, RangeOps)]
#[strict_type(lib = LIB_NAME_BITMASK)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate", transparent)
)]
pub struct OrderId(
    #[from]
    #[from([u8; 32])]
    Bytes32,
);

impl ToBaid58<32> for OrderId {
    const HRI: &'static str = "swap";
    fn to_baid58_payload(&self) -> [u8; 32] {
        self.to_byte_array()
    }
}
impl FromBaid58<32> for OrderId {}
impl Display for OrderId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if f.sign_minus() {
            write!(f, "urn:diba:{::<}", self.to_baid58())
        } else {
            write!(f, "urn:diba:{::<#}", self.to_baid58())
        }
    }
}
impl FromStr for OrderId {
    type Err = Baid58ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_baid58_str(s.trim_start_matches("urn:diba:"))
    }
}

#[deprecated(note = "removed in favor to compatibility with other wallets")]
#[derive(Clone, Debug, StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = LIB_NAME_BITMASK)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]

pub struct TransferSwap {
    pub offer_id: OrderId,
    pub bid_id: OrderId,
    pub consig: Transfer,
}

impl StrictSerialize for TransferSwap {}
impl StrictDeserialize for TransferSwap {}

impl TransferSwap {
    pub fn with(offer_id: &str, bid_id: &str, transfer: Transfer) -> Self {
        let offer_id = OrderId::from_str(offer_id).expect("Invalid rgb offer Id");
        let bid_id = OrderId::from_str(bid_id).expect("Invalid rgb bid Id");

        Self {
            offer_id,
            bid_id,
            consig: transfer,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum TransferSwapError {
    /// Consignment data have an invalid hexadecimal format.
    WrongHex,
    /// ContractID cannot be decoded. {0}
    WrongContract(String),
    /// Consignment cannot be decoded. {0}
    WrongConsig(String),
    /// Network cannot be decoded. {0}
    WrongNetwork(String),
    /// The Consignment is invalid. Details: {0:?}
    InvalidConsig(Vec<String>),
    /// The Consignment is invalid (Unexpected behavior on validation).
    Inconclusive,
}

#[deprecated(note = "removed in favor to compatibility with other wallets")]
pub fn extract_transfer(
    transfer: String,
) -> Result<(Txid, Bindle<Transfer>, OrderId, OrderId), TransferSwapError> {
    let serialized = Vec::<u8>::from_hex(&transfer).map_err(|_| TransferSwapError::WrongHex)?;
    let confined = Confined::try_from_iter(serialized.iter().copied())
        .map_err(|err| TransferSwapError::WrongConsig(err.to_string()))?;

    let transfer_swap = TransferSwap::from_strict_serialized::<{ U32 }>(confined)
        .map_err(|err| TransferSwapError::WrongConsig(err.to_string()))?;

    let transfer = transfer_swap.consig;
    for (bundle_id, _) in transfer.terminals() {
        if transfer.known_transitions_by_bundle_id(bundle_id).is_none() {
            return Err(TransferSwapError::Inconclusive);
        };
        if let Some(AnchoredBundle { anchor, bundle: _ }) = transfer.anchored_bundle(bundle_id) {
            return Ok((
                anchor.txid,
                Bindle::new(transfer),
                transfer_swap.offer_id,
                transfer_swap.bid_id,
            ));
        }
    }

    Err(TransferSwapError::Inconclusive)
}
