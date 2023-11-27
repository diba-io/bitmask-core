use amplify::confinement::{Confined, U32};
use bitcoin::Address;
use bitcoin_scripts::address::AddressCompat;
use bp::Txid;
use core::fmt::Display;
use rgb::{RgbWallet, TerminalPath};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use rgbstd::containers::{Bindle, Transfer};
use serde::{Deserialize, Serialize};

pub type RgbAccountV0 = RgbAccount;
pub type RgbTransferV0 = RgbTransfer;
pub type RgbTransfersV0 = RgbTransfers;

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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
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
    /// Remember: All contract amounts are represents in [`u64`].
    /// The [`ContractAmount`] abstract the calculation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// use bitmask_core::rgb::structs::ContractAmount;
    /// let amount = ContractAmount::new(100, 2);
    ///
    /// assert_eq!(amount.int, 100);
    /// assert_eq!(amount.fract, 0);
    /// assert_eq!(amount.clone().to_value(), 10000);
    /// assert_eq!(amount.to_string(), "100.00");
    /// ```
    pub fn new(value: u64, precision: u8) -> Self {
        let pow = 10_u64.pow(precision as u32);
        let int = match precision.cmp(&0) {
            Ordering::Less | Ordering::Equal => value,
            Ordering::Greater => value / pow,
        };

        let fract = match value.cmp(&pow) {
            Ordering::Less | Ordering::Equal => 0,
            Ordering::Greater => value - int * pow,
        };

        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Define a contract value.
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
    /// use bitmask_core::rgb::structs::ContractAmount;
    /// let amount = ContractAmount::with(2, 50, 2);
    ///
    /// assert_eq!(amount.int, 2);
    /// assert_eq!(amount.fract, 50);
    /// assert_eq!(amount.clone().to_value(), 250);
    /// assert_eq!(amount.to_string(), "2.50");
    /// ```
    pub fn with(value: u64, fract: u64, precision: u8) -> Self {
        let int = value;
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
    /// Remember: All contract amounts are represents in [`u64`].
    /// The [`ContractAmount`] abstract the calculation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// use bitmask_core::rgb::structs::ContractAmount;
    /// let amount = ContractAmount::new(1100, 3);
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 100);
    /// assert_eq!(amount.clone().to_value(), 1100);
    /// assert_eq!(amount.to_string(), "1.100");
    /// ```
    pub fn load(value: u64, precision: u8) -> Self {
        let pow = 10_u64.pow(precision as u32);
        let int = value / pow;
        let fract = value - int * pow;
        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Convert a raw u64 representation of the
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
    /// use bitmask_core::rgb::structs::ContractAmount;
    /// let amount = ContractAmount::from("1".to_string(), 3);
    ///
    /// assert_eq!(amount.int.clone(), 1);
    /// assert_eq!(amount.fract, 0);
    /// assert_eq!(amount.clone().to_value(), 1000);
    /// assert_eq!(amount.to_string().clone(), "1.000");
    /// ```
    pub fn from(value: String, precision: u8) -> Self {
        let int;
        let mut fract;

        if value.contains('.') {
            let parts = value.split('.');
            let collection: Vec<&str> = parts.collect();

            fract = collection[1].parse().unwrap();
            let precision_repr = collection[1].len() as u8;
            if precision_repr < precision {
                let pow = 10_u64.pow((precision - precision_repr) as u32);
                fract *= pow;
            }

            int = collection[0].parse().unwrap();
        } else {
            let unsafe_contract_amount = Self::load(value.parse().unwrap(), precision);
            fract = unsafe_contract_amount.fract;
            int = unsafe_contract_amount.int
        };

        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Convert a decimal string representation of the
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
    /// use bitmask_core::rgb::structs::ContractAmount;
    /// let amount = ContractAmount::from_decimal_str("1.100".to_string());
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 100);
    /// assert_eq!(amount.clone().to_value(), 1100);
    /// assert_eq!(amount.to_string(), "1.100");
    /// ```
    pub fn from_decimal_str(value: String) -> Self {
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

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display("{contract_id}:{precision}")]
pub struct ContractBoilerplate {
    pub contract_id: String,
    pub iface_id: String,
    pub precision: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct MediaMetadata {
    pub id: String,
    pub mime: String,
    pub uri: String,
    pub digest: String,
}

impl MediaMetadata {
    pub fn new(id: &str, mime: &str, uri: &str, digest: &str) -> Self {
        Self {
            id: id.to_string(),
            mime: mime.to_string(),
            uri: uri.to_string(),
            digest: digest.to_string(),
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
    pub invoices: Vec<String>,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, Default, Display,
)]
#[display(doc_comments)]

pub struct RgbTransfers {
    pub transfers: BTreeMap<String, Vec<RgbTransferV0>>,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, Default, Display,
)]
#[display(doc_comments)]

pub struct RgbTransfersV1 {
    pub transfers: BTreeMap<String, Vec<RgbTransferV1>>,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Display)]
#[display("{consig_id}:{tx}")]
pub struct RgbTransfer {
    pub consig_id: String,
    pub iface: String,
    pub consig: String,
    pub tx: Txid,
    pub is_send: bool,
}

impl Default for RgbTransfer {
    fn default() -> Self {
        Self {
            tx: Txid::coinbase(),
            consig_id: Default::default(),
            iface: Default::default(),
            consig: Default::default(),
            is_send: Default::default(),
        }
    }
}

type Beneficiary = String;
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Display)]
#[display("{consig_id}:{tx_id}")]
pub struct RgbTransferV1 {
    pub consig_id: String,
    pub tx_id: Txid,
    pub iface: String,
    pub consig: String,
    pub sender: bool,
    pub rbf: bool,
    pub utxos: Vec<String>,
    pub beneficiaries: Vec<Beneficiary>,
}

impl RgbTransferV1 {
    pub fn new(
        consig_id: String,
        consig: String,
        iface: String,
        tx_id: Txid,
        beneficiaries: Vec<String>,
    ) -> Self {
        Self {
            consig_id,
            tx_id,
            iface,
            consig,
            beneficiaries,
            sender: false,
            rbf: true,
            utxos: vec![],
        }
    }
}

impl Default for RgbTransferV1 {
    fn default() -> Self {
        Self {
            tx_id: Txid::coinbase(),
            consig_id: Default::default(),
            iface: Default::default(),
            consig: Default::default(),
            sender: Default::default(),
            rbf: Default::default(),
            utxos: vec![],
            beneficiaries: vec![],
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbExtractTransfer {
    pub consig_id: String,
    pub contract_id: String,
    pub txid: Txid,
    pub transfer: Bindle<Transfer>,
    pub strict: Confined<Vec<u8>, 0, U32>,
}

pub type RgbProxyConsigCarbonadoReq = RgbProxyCarbonadoReq<RgbProxyConsigUpload>;
pub type RgbProxyConsigUploadReq = RgbProxyUploadReq<RgbProxyConsigUpload>;
pub type RgbProxyConsigFileReq = RgbProxyFileReq<RgbProxyConsigUpload>;

pub type RgbProxyConsigUploadRes = RgbProxyRes<bool>;
pub type RgbProxyConsigRes = RgbProxyRes<RgbProxyConsig>;
pub type RgbProxyConsigErrorRes = RgbProxyErrorRes<String>;

pub type RgbProxyMediaCarbonadoReq = RgbProxyCarbonadoReq<RgbProxyMedia>;
pub type RgbProxyMediaUploadReq = RgbProxyUploadReq<RgbProxyMedia>;
pub type RgbProxyMediaUploadRes = RgbProxyRes<bool>;
pub type RgbProxyMediaFileReq = RgbProxyFileReq<RgbProxyMedia>;
pub type RgbProxyMediaReq = RgbProxyMedia;
pub type RgbProxyMediaRes = RgbProxyRes<String>;

impl From<RgbProxyConsigCarbonadoReq> for RgbProxyConsigFileReq {
    fn from(value: RgbProxyConsigCarbonadoReq) -> Self {
        let RgbProxyConsigCarbonadoReq {
            params,
            file_name,
            hex,
        } = value;

        Self {
            params,
            file_name,
            bytes: hex::decode(hex).expect("Error when parse hexadecimal data"),
        }
    }
}

impl From<RgbProxyConsigFileReq> for RgbProxyConsigCarbonadoReq {
    fn from(value: RgbProxyConsigFileReq) -> Self {
        let RgbProxyConsigFileReq {
            params,
            file_name,
            bytes,
        } = value;

        Self {
            params,
            file_name,
            hex: hex::encode(bytes),
        }
    }
}

impl From<RgbProxyMediaCarbonadoReq> for RgbProxyMediaFileReq {
    fn from(value: RgbProxyMediaCarbonadoReq) -> Self {
        let RgbProxyMediaCarbonadoReq {
            params,
            file_name,
            hex,
        } = value;

        Self {
            params,
            file_name,
            bytes: hex::decode(hex).expect("Error when parse hexadecimal data"),
        }
    }
}

impl From<RgbProxyMediaFileReq> for RgbProxyMediaCarbonadoReq {
    fn from(value: RgbProxyMediaFileReq) -> Self {
        let RgbProxyMediaFileReq {
            params,
            file_name,
            bytes,
        } = value;

        Self {
            params,
            file_name,
            hex: hex::encode(bytes),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyMedia {
    pub attachment_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyConsigReq {
    pub recipient_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]

pub struct RgbProxyRes<T> {
    pub jsonrpc: String,
    pub id: String,
    pub result: T,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]

pub struct RgbProxyErrorRes<T> {
    pub jsonrpc: String,
    pub id: String,
    pub error: T,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyFileReq<T> {
    pub params: T,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyCarbonadoReq<T> {
    pub params: T,
    pub file_name: String,
    pub hex: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyUploadReq<T> {
    pub params: T,
    pub file_name: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyConsigUpload {
    pub recipient_id: String,
    pub txid: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyConsig {
    pub consignment: String,
    pub txid: String,
}
