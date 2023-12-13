use crate::rgb::structs::{
    RgbAccountV0, RgbAccountV1, RgbTransferV0, RgbTransferV1, RgbTransfersV0, RgbTransfersV1,
};
use postcard::from_bytes;

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ModelVersionError {
    /// Version unknown. {0:?}
    Unknown(String),
    /// Decode Error. {0}
    WrongDecode(postcard::Error),
}

pub trait ModelVersion<T> {
    fn from_bytes(bytes: Vec<u8>, version: [u8; 8]) -> Result<T, ModelVersionError>;
}

#[derive(Debug, Eq, PartialEq, Clone, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbAccountVersions {
    Unknown,
    V0(RgbAccountV0),
    V1(RgbAccountV1),
}

impl From<u8> for RgbAccountVersions {
    fn from(value: u8) -> Self {
        match value {
            0 => RgbAccountVersions::V0(RgbAccountV0::default()),
            1 => RgbAccountVersions::V1(RgbAccountV1::default()),
            _ => RgbAccountVersions::Unknown,
        }
    }
}

impl From<String> for RgbAccountVersions {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "v0" | "0" | "rgbst161" | "" => RgbAccountVersions::V0(RgbAccountV0::default()),
            "v10" | "v1" | "1" => RgbAccountVersions::V1(RgbAccountV1::default()),
            _ => RgbAccountVersions::Unknown,
        }
    }
}

impl From<[u8; 8]> for RgbAccountVersions {
    fn from(value: [u8; 8]) -> Self {
        let value = String::from_utf8(value.to_vec()).expect("invalid rgb account metadata value");
        let value = value.trim_matches(char::from(0));
        RgbAccountVersions::from(value.to_string())
    }
}

impl ModelVersion<RgbAccountV1> for RgbAccountVersions {
    fn from_bytes(bytes: Vec<u8>, version: [u8; 8]) -> Result<RgbAccountV1, ModelVersionError> {
        let target_version = RgbAccountVersions::from(version);
        let latest_version = match target_version {
            RgbAccountVersions::Unknown => {
                return Err(ModelVersionError::Unknown(
                    String::from_utf8(version.to_vec()).unwrap(),
                ))
            }
            RgbAccountVersions::V0(mut previous_version) => {
                previous_version = from_bytes(&bytes).map_err(ModelVersionError::WrongDecode)?;
                RgbAccountV1::from(previous_version)
            }
            RgbAccountVersions::V1(mut current_version) => {
                current_version = from_bytes(&bytes).map_err(ModelVersionError::WrongDecode)?;
                current_version
            }
        };

        Ok(latest_version)
    }
}

impl From<RgbAccountV0> for RgbAccountV1 {
    fn from(value: RgbAccountV0) -> Self {
        Self {
            wallets: value.wallets,
            ..Default::default()
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbtransferVersions {
    Unknown,
    V0(RgbTransfersV0),
    V1(RgbTransfersV1),
}

impl From<String> for RgbtransferVersions {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "v0" | "0" | "rgbst161" | "" => RgbtransferVersions::V0(RgbTransfersV0::default()),
            "v10" | "v1" | "1" => RgbtransferVersions::V1(RgbTransfersV1::default()),
            _ => RgbtransferVersions::Unknown,
        }
    }
}

impl From<[u8; 8]> for RgbtransferVersions {
    fn from(value: [u8; 8]) -> Self {
        let value = String::from_utf8(value.to_vec()).expect("invalid rgb account metadata value");
        let value = value.trim_matches(char::from(0));
        RgbtransferVersions::from(value.to_string())
    }
}

impl ModelVersion<RgbTransfersV1> for RgbtransferVersions {
    fn from_bytes(bytes: Vec<u8>, version: [u8; 8]) -> Result<RgbTransfersV1, ModelVersionError> {
        let target_version = RgbtransferVersions::from(version);
        let latest_version = match target_version {
            RgbtransferVersions::Unknown => {
                return Err(ModelVersionError::Unknown(
                    String::from_utf8(version.to_vec()).unwrap(),
                ))
            }
            RgbtransferVersions::V0(mut previous_version) => {
                previous_version = from_bytes(&bytes).map_err(ModelVersionError::WrongDecode)?;
                RgbTransfersV1::from(previous_version)
            }
            RgbtransferVersions::V1(mut current_version) => {
                current_version = from_bytes(&bytes).map_err(ModelVersionError::WrongDecode)?;
                current_version
            }
        };

        Ok(latest_version)
    }
}

impl From<RgbTransfersV0> for RgbTransfersV1 {
    fn from(value: RgbTransfersV0) -> Self {
        let mut transfers = RgbTransfersV1::default();
        for (k, v) in value.transfers {
            let map = v.into_iter().map(RgbTransferV1::from).collect();
            transfers.transfers.insert(k, map);
        }
        transfers
    }
}

impl From<RgbTransferV0> for RgbTransferV1 {
    fn from(value: RgbTransferV0) -> Self {
        let RgbTransferV0 {
            consig_id,
            iface,
            consig,
            tx: tx_id,
            is_send: sender,
        } = value;

        Self {
            consig_id,
            tx_id,
            iface,
            consig,
            sender,
            rbf: false,
            utxos: vec![],
            beneficiaries: vec![],
        }
    }
}
