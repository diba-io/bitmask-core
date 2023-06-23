use std::{collections::HashMap, str::FromStr};

use bitcoin::Address;
use bitcoin_scripts::address::AddressCompat;
use bp::Outpoint;
use rgb::{interface::OutpointFilter, RgbWallet, TerminalPath};

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

pub struct EmptyFilter {}
impl OutpointFilter for EmptyFilter {
    fn include_outpoint(&self, _outpoint: Outpoint) -> bool {
        true
    }
}
