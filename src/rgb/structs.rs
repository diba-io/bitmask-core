use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use amplify::confinement::{Confined, U32};
use bitcoin::Address;
use bitcoin_scripts::address::AddressCompat;
use bp::Txid;
use rgb::{RgbWallet, TerminalPath};

use rgbstd::containers::{Bindle, Transfer};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display("{address}:{amount}", alt = "{address:#}:{amount:#}")]
pub struct AddressAmount {
    pub address: Address,
    pub amount: u64,
}

/// Error parsing representation
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AddressFormatParseError;

impl FromStr for AddressAmount {
    type Err = AddressFormatParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split: Vec<&str> = s.split(':').collect();
        let address = Address::from_str(split[0]).expect("invalid address format");
        let amount = u64::from_str(split[1]).expect("invalid address format");
        Ok(AddressAmount { address, amount })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RgbAccount {
    pub wallets: HashMap<String, RgbWallet>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display("{address}")]
pub struct AddressTerminal {
    pub address: AddressCompat,
    pub terminal: TerminalPath,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, Default, Display,
)]
#[display(doc_comments)]

pub struct RgbTransfers {
    pub transfers: BTreeMap<String, Vec<RgbTransfer>>,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Display)]
#[display("{tx}")]
pub struct RgbTransfer {
    pub consig_id: String,
    pub iface: String,
    pub consig: String,
    pub tx: Txid,
    pub is_send: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbExtractTransfer {
    pub consig_id: String,
    pub contract_id: String,
    pub tx_id: Txid,
    pub transfer: Bindle<Transfer>,
    pub strict: Confined<Vec<u8>, 0, U32>,
    pub offer_id: Option<String>,
    pub bid_id: Option<String>,
}
