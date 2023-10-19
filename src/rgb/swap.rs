use super::{
    constants::LIB_NAME_BITMASK,
    crdt::{LocalRgbOfferBid, LocalRgbOffers},
    fs::{
        retrieve_public_offers, retrieve_swap_offer_bid, store_public_offers, store_swap_bids,
        RgbPersistenceError,
    },
};
use crate::{structs::AllocationDetail, validators::RGBContext};
use amplify::{
    confinement::{Confined, U32},
    hex::{FromHex, ToHex},
    Array, ByteArray, Bytes32,
};
use autosurgeon::{reconcile, Hydrate, Reconcile};
use baid58::{Baid58ParseError, FromBaid58, ToBaid58};
use bitcoin::psbt::Psbt;
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

#[derive(Clone, Serialize, Deserialize, Validate, Reconcile, Hydrate, Debug, Display, Default)]
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
    #[garde(ascii)]
    pub terminal: String,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub asset_precision: u8,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_price: u64,
    #[garde(ascii)]
    pub seller_psbt: String,
    #[garde(ascii)]
    pub seller_address: String,
    #[garde(skip)]
    pub expire_at: Option<i64>,
    #[garde(ascii)]
    pub public: String,
    #[garde(skip)]
    pub presig: bool,
    #[garde(skip)]
    pub transfer_id: Option<String>,
}

impl RgbOffer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        secret: String,
        contract_id: String,
        iface: String,
        allocations: Vec<AllocationDetail>,
        asset_precision: u8,
        seller_address: AddressCompat,
        bitcoin_price: u64,
        psbt: String,
        presig: bool,
        terminal: String,
        expire_at: Option<i64>,
    ) -> Self {
        let secp = Secp256k1::new();
        let secret = hex::decode(secret).expect("cannot decode hex sk in new RgbOffer");
        let secret_key = SecretKey::from_slice(&secret).expect("error parsing sk in new RgbOffer");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

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

        let mut hasher = blake3::Hasher::new();
        for asset_utxo in asset_utxos {
            hasher.update(asset_utxo.as_bytes());
        }

        let id = Array::from_array(hasher.finalize().into());
        let order_id = OrderId(id);
        let order_id = order_id.to_baid58_string();

        RgbOffer {
            offer_id: order_id.to_string(),
            offer_status: RgbOrderStatus::Open,
            contract_id,
            iface,
            asset_amount,
            asset_precision,
            bitcoin_price,
            seller_psbt: psbt,
            seller_address: seller_address.to_string(),
            public: public_key.to_hex(),
            presig,
            expire_at,
            terminal,
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
    pub seller_psbt: String,
    #[garde(ascii)]
    pub seller_address: String,
    #[garde(skip)]
    pub expire_at: Option<i64>,
    #[garde(ascii)]
    pub public: String,
    #[garde(skip)]
    pub presig: bool,
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
            public,
            expire_at,
            presig,
            asset_precision,
            ..
        } = value;

        Self {
            offer_id,
            contract_id,
            iface,
            asset_amount,
            bitcoin_price,
            seller_psbt,
            seller_address,
            public,
            expire_at,
            presig,
            asset_precision,
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
    #[garde(skip)]
    pub iface: String,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub asset_amount: u64,
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub asset_precision: u8,
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_amount: u64,
    #[garde(ascii)]
    pub buyer_psbt: String,
    #[garde(ascii)]
    pub buyer_invoice: String,
    #[garde(ascii)]
    pub public: String,
    #[garde(skip)]
    pub transfer_id: Option<String>,
    #[garde(skip)]
    pub transfer: Option<String>,
}

impl RgbBid {
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
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        let mut allocations = bitcoin_utxos;
        allocations.sort();

        let mut hasher = blake3::Hasher::new();
        for allocation in allocations {
            hasher.update(allocation.as_bytes());
        }

        let id = Array::from_array(hasher.finalize().into());
        let order_id = OrderId(id);
        let order_id = order_id.to_baid58_string();

        RgbBid {
            bid_id: order_id.to_string(),
            bid_status: RgbOrderStatus::Open,
            offer_id,
            contract_id,
            asset_amount,
            asset_precision,
            bitcoin_amount: bitcoin_price,
            public: public_key.to_hex(),
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
    pub buyer_psbt: String,
    #[garde(ascii)]
    pub buyer_invoice: String,
    #[garde(ascii)]
    pub public: String,
    #[garde(skip)]
    pub transfer_id: Option<String>,
    #[garde(skip)]
    pub transfer: Option<String>,
    #[garde(skip)]
    pub tap_outpoint: Option<String>,
    #[garde(skip)]
    pub tap_commit: Option<String>,
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
            public,
            transfer_id,
            transfer,
            iface,
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
            public,
            transfer_id,
            transfer,
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
    pub public: String,
}

impl From<RgbBidSwap> for PublicRgbBid {
    fn from(value: RgbBidSwap) -> Self {
        let RgbBidSwap {
            bid_id,
            asset_amount,
            bitcoin_amount,
            public,
            ..
        } = value;

        Self {
            bid_id,
            asset_amount,
            bitcoin_amount,
            public,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbOffers {
    pub offers: BTreeMap<AssetId, Vec<RgbOffer>>,
    pub bids: BTreeMap<OfferId, BTreeMap<BidId, RgbBid>>,
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct RgbBids {
    pub bids: BTreeMap<AssetId, Vec<RgbBid>>,
}

#[derive(Clone, Serialize, Deserialize, Reconcile, Hydrate, Default, Debug)]
pub struct PublicRgbOffers {
    pub offers: BTreeMap<AssetId, Vec<RgbOfferSwap>>,
    pub bids: BTreeMap<OfferId, BTreeMap<BidId, PublicRgbBid>>,
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
    /// Occurs an error in merge step. {0}
    AutoMerge(String),
}

pub async fn get_public_offer(offer_id: OfferId) -> Result<RgbOfferSwap, RgbOfferErrors> {
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

pub async fn get_public_bid(
    offer_id: OfferId,
    bid_id: BidId,
) -> Result<PublicRgbBid, RgbOfferErrors> {
    let LocalRgbOffers { doc: _, rgb_offers } =
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

pub async fn get_swap_bids_by_seller(
    sk: &str,
    offer: RgbOffer,
) -> Result<Vec<RgbBidSwap>, RgbOfferErrors> {
    let RgbOffer {
        offer_id,
        expire_at,
        ..
    } = offer;

    let LocalRgbOffers { doc: _, rgb_offers } =
        retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let public_bids: Vec<PublicRgbBid> = match rgb_offers.bids.get(&offer_id) {
        Some(bids) => bids.values().cloned().collect(),
        _ => return Err(RgbOfferErrors::NoOffer(offer_id)),
    };

    let mut swap_bids = vec![];
    for bid in public_bids {
        let PublicRgbBid { bid_id, public, .. } = bid.clone();
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

pub async fn get_swap_bid(
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
        PublicKey::from_str(&bid.public).map_err(|op| RgbOfferErrors::Keys(op.to_string()))?;

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
        expire_at, public, ..
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

pub async fn publish_public_offer(new_offer: RgbOfferSwap) -> Result<(), RgbOfferErrors> {
    let LocalRgbOffers {
        doc,
        mut rgb_offers,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&doc)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;
    if let Some(offers) = rgb_offers.offers.get(&new_offer.contract_id) {
        let mut avaliable_offers = offers.to_owned();
        if let Some(position) = avaliable_offers
            .iter()
            .position(|x| x.offer_id == new_offer.offer_id)
        {
            avaliable_offers.remove(position);
            avaliable_offers.insert(position, new_offer.clone());
        } else {
            avaliable_offers.push(new_offer.clone());
        }

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

pub async fn publish_public_bid(new_bid: RgbBidSwap) -> Result<(), RgbOfferErrors> {
    let RgbBidSwap {
        bid_id, offer_id, ..
    } = new_bid.clone();

    let _ = get_public_offer(offer_id.clone()).await?;
    let LocalRgbOffers {
        doc,
        mut rgb_offers,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&doc)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    let new_public_bid = PublicRgbBid::from(new_bid);
    if let Some(bids) = rgb_offers.bids.get(&offer_id) {
        let mut avaliable_bids = bids.to_owned();
        avaliable_bids.insert(bid_id, new_public_bid);
        rgb_offers.bids.insert(offer_id.clone(), avaliable_bids);
    } else {
        rgb_offers
            .bids
            .insert(offer_id.clone(), bmap! { bid_id => new_public_bid });
    }

    // TODO: Add change verification (accept only addition operation)
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
    let LocalRgbOfferBid { doc, .. } = retrieve_swap_offer_bid(&share_sk, &file_name, expire_at)
        .await
        .map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&doc)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    reconcile(&mut local_copy, new_bid).map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_swap_bids(&share_sk, &file_name, local_copy.save(), expire_at)
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
}

pub async fn remove_public_offers(offers: Vec<RgbOffer>) -> Result<(), RgbOfferErrors> {
    let LocalRgbOffers {
        doc,
        mut rgb_offers,
    } = retrieve_public_offers().await.map_err(RgbOfferErrors::IO)?;

    let mut local_copy = automerge::AutoCommit::load(&doc)
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

pub async fn mark_transfer_offer(
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

pub async fn mark_transfer_bid(
    bid_id: BidId,
    consig_id: TransferId,
    rgb_bids: &mut RgbBids,
) -> Result<(), RgbOfferErrors> {
    let bids = rgb_bids.bids.clone();
    for (contract_id, mut my_bids) in bids {
        if let Some(position) = my_bids.iter().position(|x| x.bid_id == bid_id) {
            let mut offer = my_bids.swap_remove(position);
            offer.transfer_id = Some(consig_id.to_owned());
            // offer.transfer = transfer;

            my_bids.insert(position, offer);
            rgb_bids.bids.insert(contract_id, my_bids);
            break;
        }
    }
    Ok(())
}

pub async fn mark_offer_fill(
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
            offer.offer_status = RgbOrderStatus::Fill;

            offer_filled = Some(offer.clone());
            my_offers.insert(position, offer);
            rgb_offers.offers.insert(contract_id, my_offers);

            break;
        }
    }
    Ok(offer_filled)
}

pub async fn mark_bid_fill(
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
            bid.bid_status = RgbOrderStatus::Fill;

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

impl PsbtSwapEx<Psbt> for Psbt {
    type Error = PsbtSwapExError;

    fn join(self, other: Psbt) -> Result<Psbt, Self::Error> {
        // BIP 174: The Combiner must remove any duplicate key-value pairs, in accordance with
        //          the specification. It can pick arbitrarily when conflicts occur.

        // Keeping the highest version
        let mut new_psbt = Psbt::from(self).clone();
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

        // TODO: Make more tests!
        // new_psbt.inputs.remove(0);
        // new_psbt.inputs.insert(0, other.inputs.remove(0));
        new_psbt.inputs.extend(other.inputs);

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

        // new_psbt.unsigned_tx.input.remove(0);
        // new_psbt
        //     .unsigned_tx
        //     .input
        //     .insert(0, other.unsigned_tx.input.remove(0));
        new_psbt.unsigned_tx.input.extend(other.unsigned_tx.input);

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
