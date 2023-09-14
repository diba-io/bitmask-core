use autosurgeon::{reconcile, Hydrate, Reconcile};
use bitcoin::psbt::{PartiallySignedTransaction, Psbt};
use bitcoin_scripts::address::AddressCompat;
use garde::Validate;
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    collections::{btree_map, BTreeMap},
};

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
    pub buyer_invoice: String,
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
    pub bids: BTreeMap<OfferId, BTreeMap<BidId, RgbBid>>,
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
    NoBid(String),
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

pub async fn get_public_bid(offer_id: OfferId, bid_id: BidId) -> Result<RgbBid, RgbOfferErrors> {
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

    let RgbBid {
        bid_id, offer_id, ..
    } = new_bid.clone();

    if let Some(bids) = rgb_offers.bids.get(&new_bid.offer_id) {
        let mut avaliable_bids = bids.to_owned();
        avaliable_bids.insert(bid_id, new_bid);
        rgb_offers.bids.insert(offer_id.clone(), avaliable_bids);
    } else {
        rgb_offers
            .bids
            .insert(offer_id.clone(), bmap! { bid_id => new_bid });
    }

    // TODO: Add change verification (accept only addition operation)
    reconcile(&mut local_copy, rgb_offers)
        .map_err(|op| RgbOfferErrors::AutoMerge(op.to_string()))?;

    store_public_offers(local_copy.save())
        .await
        .map_err(RgbOfferErrors::IO)?;

    Ok(())
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

impl PsbtSwapEx<PartiallySignedTransaction> for PartiallySignedTransaction {
    type Error = PsbtSwapExError;

    fn join(
        self,
        other: PartiallySignedTransaction,
    ) -> Result<PartiallySignedTransaction, Self::Error> {
        // BIP 174: The Combiner must remove any duplicate key-value pairs, in accordance with
        //          the specification. It can pick arbitrarily when conflicts occur.

        // Keeping the highest version
        let mut new_psbt = Psbt::from(self).clone();
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

        new_psbt.inputs.extend(other.inputs);
        new_psbt.outputs.extend(other.outputs);

        // Transaction
        new_psbt.unsigned_tx.version =
            cmp::max(new_psbt.unsigned_tx.version, other.unsigned_tx.version);

        new_psbt.unsigned_tx.lock_time =
            cmp::max(new_psbt.unsigned_tx.lock_time, other.unsigned_tx.lock_time);

        new_psbt.unsigned_tx.input.extend(other.unsigned_tx.input);
        new_psbt.unsigned_tx.output.extend(other.unsigned_tx.output);

        Ok(new_psbt.clone())
    }
}
