/*
use std::str::FromStr;

use anyhow::Result;
use bitcoin::secp256k1::Secp256k1;
use bitcoin_hd::DerivationAccount;
use miniscript::{Descriptor, MiniscriptKey};
use wallet::hd::{Descriptor as DescrTrait, UnhardenedIndex};

use crate::{debug, info};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Display, From)]
#[display(inner)]
#[allow(clippy::large_enum_variant)]
pub enum DerivationRef {
    #[from]
    TrackingAccount(DerivationAccount),
    #[from]
    NamedAccount(String),
}

impl FromStr for DerivationRef {
    type Err = bitcoin_hd::account::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s.contains(&['[', '{', '/', '*']) {
            DerivationRef::TrackingAccount(DerivationAccount::from_str(s)?)
        } else {
            DerivationRef::NamedAccount(s.to_owned())
        })
    }
}

impl MiniscriptKey for DerivationRef {
    type Hash = Self;

    #[inline]
    fn to_pubkeyhash(&self) -> Self::Hash {
        self.clone()
    }
}

// descriptor-wallet -> bin/btc-cold -> Args::address()
pub fn rgb_address(descriptor_str: &str, index: u16, change: bool) -> Result<String> {
    info!("Deriving RGB descriptor wallet addresses");
    debug!(format!("Descriptor provided: {descriptor_str}"));

    let secp = Secp256k1::new();
    let descriptor = Descriptor::<DerivationAccount>::from_str(descriptor_str)?;
    debug!("Descriptor successfully imported");
    let address = DescrTrait::<bitcoin::PublicKey>::address(
        &descriptor,
        &secp,
        &[
            UnhardenedIndex::from(if change { 1u8 } else { 0u8 }),
            UnhardenedIndex::from(index),
        ],
    )?;

    let address = address.to_string();

    info!(format!("RGB address for index {index}'"));

    Ok(address)
}
*/
