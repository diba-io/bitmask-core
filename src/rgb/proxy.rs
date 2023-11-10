use std::collections::BTreeMap;

use amplify::confinement::U32;
use postcard::from_bytes;
use strict_encoding::StrictSerialize;

use crate::proxy::{
    proxy_consig_retrieve, proxy_consig_store, proxy_media_data_store, proxy_media_retrieve,
    proxy_metadata_retrieve,
};

use crate::proxy::ProxyServerError;
use crate::structs::{MediaEncode, MediaItemRequest};

use super::{
    structs::{MediaMetadata, RgbProxyConsigFileReq, RgbProxyConsigUpload},
    transfer::extract_transfer,
};

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ProxyError {
    /// Proxy Server error. {0}
    IO(ProxyServerError),
    /// Consignment cannot be decoded. {0}
    WrongConsig(String),
    /// Retrieve '{0}' serialize causes error. {1}
    SerializeRetrieve(String, String),
    /// Write '{0}' serialize causes error. {1}
    SerializeWrite(String, String),
}

pub async fn post_consignments(consignments: BTreeMap<String, String>) -> Result<(), ProxyError> {
    for (recipient_id, transfer) in consignments {
        let hashed_name = blake3::hash(recipient_id.as_bytes())
            .to_hex()
            .to_lowercase();

        let (txid, transfer) =
            extract_transfer(transfer).map_err(|op| ProxyError::WrongConsig(op.to_string()))?;
        let bytes = transfer
            .unbindle()
            .to_strict_serialized::<U32>()
            .map_err(|op| ProxyError::WrongConsig(op.to_string()))?;

        let consig_rq = RgbProxyConsigFileReq {
            params: RgbProxyConsigUpload {
                recipient_id,
                txid: txid.to_string(),
            },
            bytes: bytes.to_vec(),
            file_name: hashed_name,
        };

        let _ = proxy_consig_store(consig_rq).await.map_err(ProxyError::IO);
    }

    Ok(())
}

pub async fn get_consignment(consig_or_receipt_id: &str) -> Result<Option<String>, ProxyError> {
    let resp = proxy_consig_retrieve(consig_or_receipt_id)
        .await
        .map_err(ProxyError::IO)?;

    if resp.is_none() {
        return Ok(None);
    }

    let bytes = &base64::decode(&resp.unwrap().result.consignment).map_err(|op| {
        ProxyError::SerializeRetrieve("consignment.get".to_string(), op.to_string())
    })?;

    if bytes.is_empty() {
        Ok(None)
    } else {
        Ok(Some(hex::encode(bytes)))
    }
}

pub async fn get_media(media_id: &str) -> Result<Option<Vec<u8>>, ProxyError> {
    let resp = proxy_media_retrieve(media_id)
        .await
        .map_err(ProxyError::IO)?;

    if resp.is_none() {
        return Ok(None);
    }

    let bytes = base64::decode(&resp.unwrap().result)
        .map_err(|op| ProxyError::SerializeRetrieve("media".to_string(), op.to_string()))?;

    if bytes.is_empty() {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}

pub async fn get_media_metadata(media_id: &str) -> Result<Option<MediaMetadata>, ProxyError> {
    let resp = proxy_metadata_retrieve(&format!("{media_id}-metadata"))
        .await
        .map_err(ProxyError::IO)?;

    if resp.is_none() {
        return Ok(None);
    }

    let bytes = base64::decode(&resp.unwrap().result)
        .map_err(|op| ProxyError::SerializeRetrieve("metadata".to_string(), op.to_string()))?;

    if bytes.is_empty() {
        Ok(None)
    } else {
        let metadata: MediaMetadata = from_bytes(&bytes).map_err(|op| {
            ProxyError::SerializeRetrieve("metadata.postcard".to_string(), op.to_string())
        })?;

        Ok(Some(metadata))
    }
}

pub async fn post_media_metadata(
    data: MediaItemRequest,
    encode: MediaEncode,
) -> Result<MediaMetadata, ProxyError> {
    let data = proxy_media_data_store(data, encode)
        .await
        .map_err(ProxyError::IO)?;

    Ok(data)
}

pub async fn post_media_metadata_list(
    data: Vec<MediaItemRequest>,
    encode: MediaEncode,
) -> Result<Vec<MediaMetadata>, ProxyError> {
    let mut list = vec![];
    for item in data {
        let data: MediaMetadata = proxy_media_data_store(item, encode.clone())
            .await
            .map_err(ProxyError::IO)?;

        list.push(data);
    }

    Ok(list)
}
