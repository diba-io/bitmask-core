use amplify::confinement::{Confined, U32};
use bitcoin::Address;
use bitcoin_scripts::address::AddressCompat;
use bp::Txid;
use core::fmt::Display;
use rgb::{RgbWallet, TerminalPath};
use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use rgbstd::containers::{Bindle, Transfer};
use serde::{Deserialize, Serialize};

pub type RgbAccountV0 = RgbAccount;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display("{address}:{amount}", alt = "{address:#}:{amount:#}")]
pub struct AddressAmount {
    pub address: Address,
    pub amount: u64,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display("{address}")]
pub struct AddressTerminal {
    pub address: AddressCompat,
    pub terminal: TerminalPath,
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

pub struct ContractAmount {
    pub int: u64,
    pub fract: u64,
    pub precision: u8,
}

impl ContractAmount {
    /// Initialize a new contract value.
    ///
    /// A value ([`u64`]) combine with precision ([`u8`]), generate
    /// a new contract value, compabitle with rgb contract value.
    ///
    /// Remeber: All contract amounts are represents in [`u64`].
    /// The [`ContractAmount`] abstract the calculation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// let amount = CoinAmount::with(5, 2);
    ///
    /// assert_eq!(amount.int, 5);
    /// assert_eq!(amount.fract, 0);
    /// assert_eq!(amount.to_value(), 500);
    /// assert_eq!(amount.to_string(), "5");
    /// ```
    pub fn new(value: u64, precision: u8) -> Self {
        let pow = 10_u64.pow(precision as u32);

        let int = if value < pow { value } else { value / pow };
        let fract = if value < pow { 0 } else { value - int * pow };
        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Load a contract value.
    ///
    /// A value ([`u64`]) combine with precision ([`u8`]), load
    /// a contract value, compabitle with rgb contract value.
    ///
    /// Remeber: All contract amounts are represents in [`u64`].
    /// The [`ContractAmount`] abstract the calculation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// let amount = ContractAmount::with(1100, 3);
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 100);
    /// assert_eq!(amount.to_value(), 1100);
    /// assert_eq!(amount.to_string(), "1.100");
    /// ```
    pub fn with(value: u64, precision: u8) -> Self {
        let pow = 10_u64.pow(precision as u32);
        let int = value / pow;
        let fract = value - int * pow;
        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Convert a raw string representation of the
    /// number in Contract Amount.
    ///
    /// A value ([`String`]) return a contract value,
    /// compabitle with rgb contract value.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// let amount = ContractAmount::from("1.100");
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 100);
    /// assert_eq!(amount.to_value(), 1100);
    /// assert_eq!(amount.to_string(), "1.100");
    /// ```
    pub fn from(value: String, precision: u8) -> Self {
        let mut fract = 0;

        let int = if value.contains('.') {
            let parts = value.split('.');
            let collection: Vec<&str> = parts.collect();

            fract = collection[1].parse().unwrap();
            let precision_repr = collection[1].len() as u8;
            if precision_repr < precision {
                let pow = 10_u64.pow((precision - precision_repr) as u32);
                fract *= pow;
            }

            collection[0].parse().unwrap()
        } else {
            value.parse().unwrap()
        };

        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Convert a raw string representation of the
    /// number in Contract Amount.
    ///
    /// A value ([`String`]) return a contract value,
    /// compabitle with rgb contract value.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// let amount = ContractAmount::from_raw("1.100");
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 100);
    /// assert_eq!(amount.to_value(), 1100);
    /// assert_eq!(amount.to_string(), "1.100");
    /// ```
    pub fn from_raw(value: String) -> Self {
        let mut fract = 0;
        let mut precision = 0;

        let int = if value.contains('.') {
            let parts = value.split('.');
            let collection: Vec<&str> = parts.collect();

            fract = collection[1].parse().unwrap();
            precision = collection[1].len() as u8;

            collection[0].parse().unwrap()
        } else {
            value.parse().unwrap()
        };

        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    pub fn to_value(self) -> u64 {
        let pow = 10_u64.pow(self.precision as u32);
        self.int * pow + self.fract
    }
}

impl Display for ContractAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let precision = self.precision as usize;
        if precision > 0 {
            write!(f, "{}.{:0precision$}", self.int, self.fract)
        } else {
            write!(f, "{}", self.int)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct RgbAccount {
    pub wallets: HashMap<String, RgbWallet>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct RgbAccountV1 {
    pub wallets: HashMap<String, RgbWallet>,
    pub hidden_contracts: Vec<String>,
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
