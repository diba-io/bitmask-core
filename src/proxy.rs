#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ProxyServerError {
    /// I/O or connectivity error. {0}
    IO(String),
    /// Server connectivity error. {0}
    Server(String),
    /// JSON RPC Parse error. {0}
    Parse(String),
    /// All endpoints failed error
    AllEndpointsFailed,
}

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    handle_file, proxy_consig_retrieve, proxy_consig_store, proxy_media_retrieve, proxy_media_store,
};
#[cfg(not(target_arch = "wasm32"))]
mod server {
    use reqwest::multipart::{self, Part};
    use std::path::PathBuf;
    use tokio::fs;

    use crate::{
        constants::{NETWORK, RGB_PROXY_ENDPOINT},
        info,
        rgb::structs::{
            RgbProxyConsigFileReq, RgbProxyConsigReq, RgbProxyConsigRes, RgbProxyConsigUploadReq,
            RgbProxyConsigUploadRes, RgbProxyMediaFileReq, RgbProxyMediaReq, RgbProxyMediaRes,
            RgbProxyMediaUploadReq, RgbProxyMediaUploadRes,
        },
        util::{post_data, upload_data},
    };

    use super::ProxyServerError;

    pub async fn proxy_consig_store(
        request: RgbProxyConsigFileReq,
    ) -> Result<RgbProxyConsigUploadRes, ProxyServerError> {
        let RgbProxyConsigFileReq {
            file_name,
            bytes,
            params,
        } = request;

        let filepath = handle_file(&file_name, bytes.len())
            .await
            .map_err(|op| ProxyServerError::Server(op.to_string()))?;

        fs::write(filepath.clone(), bytes)
            .await
            .map_err(|op| ProxyServerError::Server(op.to_string()))?;

        let request_data = RgbProxyConsigUploadReq {
            params,
            file_name: filepath.to_string_lossy().to_string(),
        };

        let resp = fetch_consignment_post(request_data).await?;
        fs::remove_file(filepath)
            .await
            .map_err(|op| ProxyServerError::Server(op.to_string()))?;

        Ok(resp)
    }

    pub async fn proxy_media_store(
        request: RgbProxyMediaFileReq,
    ) -> Result<RgbProxyMediaUploadRes, ProxyServerError> {
        let RgbProxyMediaFileReq {
            file_name,
            bytes,
            params,
        } = request;

        let filepath = handle_file(&file_name, bytes.len())
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        fs::write(filepath.clone(), bytes)
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        let request_data = RgbProxyMediaUploadReq {
            params,
            file_name: filepath.to_string_lossy().to_string(),
        };

        let resp = fetch_media_post(request_data).await?;
        fs::remove_file(filepath)
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        Ok(resp)
    }

    pub async fn proxy_consig_retrieve(
        request_id: String,
    ) -> Result<Option<RgbProxyConsigRes>, ProxyServerError> {
        fetch_consignment_get(request_id).await
    }

    pub async fn proxy_media_retrieve(
        attachment_id: String,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        fetch_media_get(attachment_id).await
    }

    pub async fn handle_file(name: &str, bytes: usize) -> Result<PathBuf, ProxyServerError> {
        let mut final_name = name.to_string();
        let network = NETWORK.read().await.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = std::path::Path::new(
            &std::env::var("RGB_PROXY_DIR").unwrap_or("/tmp/bitmaskd/proxy".to_owned()),
        )
        .join(final_name);

        let filedir = filepath.parent().unwrap();
        fs::create_dir_all(filedir)
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        if bytes == 0 {
            info!(format!("read {}", filepath.to_string_lossy()));
        } else {
            info!(format!(
                "write {bytes} bytes to {}",
                filepath.to_string_lossy()
            ));
        }

        Ok(filepath)
    }

    async fn fetch_consignment_post(
        request: RgbProxyConsigUploadReq,
    ) -> Result<RgbProxyConsigUploadRes, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let file_info = fs::read(request.file_name.clone())
            .await
            .unwrap_or_default();

        let params = request.params;
        let body = serde_json::to_string(&params).unwrap_or_default();
        let form = multipart::Form::new()
            .text("method", "consignment.post")
            .text("jsonrpc", "2.0")
            .text("id", params.recipient_id)
            .text("params", body)
            .part("file", Part::bytes(file_info).file_name(request.file_name));

        let (resp, _) = upload_data(&url, form).await.map_err(|_| {
            ProxyServerError::Server(format!("Error sending JSON POST request to {url}"))
        })?;

        let resp =
            serde_json::from_str(&resp).map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(resp)
    }

    async fn fetch_consignment_get(
        recipient_id: String,
    ) -> Result<Option<RgbProxyConsigRes>, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let body = serde_json::to_string(&RgbProxyConsigReq {
            recipient_id: recipient_id.clone(),
        })
        .unwrap_or_default();
        let form = multipart::Form::new()
            .text("method", "consignment.get")
            .text("jsonrpc", "2.0")
            .text("id", recipient_id)
            .text("params", body);

        let (resp, _) = post_data(&url, form).await.map_err(|_| {
            ProxyServerError::Server(format!("Error sending JSON POST request to {url}"))
        })?;

        let resp = match serde_json::from_str(&resp) {
            Ok(resp) => Some(resp),
            Err(_) => None,
        };
        Ok(resp)
    }

    async fn fetch_media_post(
        request: RgbProxyMediaUploadReq,
    ) -> Result<RgbProxyMediaUploadRes, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let file_info = fs::read(request.file_name.clone())
            .await
            .unwrap_or_default();
        let params = request.params;
        let body = serde_json::to_string(&params).unwrap_or_default();
        let form = multipart::Form::new()
            .text("method", "media.post")
            .text("jsonrpc", "2.0")
            .text("id", params.attachment_id)
            .text("params", body)
            .part("file", Part::bytes(file_info).file_name(request.file_name));

        let (resp, _) = upload_data(&url, form).await.map_err(|_| {
            ProxyServerError::Server(format!("Error sending JSON POST request to {url}"))
        })?;

        let resp =
            serde_json::from_str(&resp).map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(resp)
    }

    async fn fetch_media_get(
        attachment_id: String,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let body = serde_json::to_string(&RgbProxyMediaReq {
            attachment_id: attachment_id.clone(),
        })
        .unwrap_or_default();
        let form = multipart::Form::new()
            .text("method", "media.get")
            .text("jsonrpc", "2.0")
            .text("id", attachment_id)
            .text("params", body);

        let (resp, _) = post_data(&url, form).await.map_err(|_| {
            ProxyServerError::Server(format!("Error sending JSON POST request to {url}"))
        })?;

        let resp = match serde_json::from_str(&resp) {
            Ok(resp) => Some(resp),
            Err(_) => None,
        };

        Ok(resp)
    }
}

#[cfg(target_arch = "wasm32")]
pub use client::{
    proxy_consig_retrieve, proxy_consig_store, proxy_media_retrieve, proxy_media_store,
};

#[cfg(target_arch = "wasm32")]
mod client {
    use crate::{
        constants::{BITMASK_ENDPOINT, NETWORK},
        rgb::structs::{
            RgbProxyConsigCarbonadoReq, RgbProxyConsigFileReq, RgbProxyConsigRes,
            RgbProxyConsigUploadRes, RgbProxyMediaCarbonadoReq, RgbProxyMediaFileReq,
            RgbProxyMediaRes, RgbProxyMediaUploadRes,
        },
        util::{get, post_json},
    };

    use super::ProxyServerError;

    pub async fn proxy_consig_store(
        request: RgbProxyConsigFileReq,
    ) -> Result<RgbProxyConsigUploadRes, ProxyServerError> {
        let network = NETWORK.read().await.to_string();
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();

        let name = request.clone().file_name;
        let url = format!("{endpoint}/proxy/consignment/{network}-{name}");
        let body = RgbProxyConsigCarbonadoReq::from(request);
        let (reponse, _) = post_json(&url, &body.clone())
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        let result = serde_json::from_str::<RgbProxyConsigUploadRes>(&reponse)
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(result.clone())
    }

    pub async fn proxy_media_store(
        request: RgbProxyMediaFileReq,
    ) -> Result<RgbProxyMediaUploadRes, ProxyServerError> {
        let network = NETWORK.read().await.to_string();
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();

        let name = request.clone().file_name;
        let url = format!("{endpoint}/proxy/media/{network}-{name}");
        let body = RgbProxyMediaCarbonadoReq::from(request);
        let (reponse, _) = post_json(&url, &body.clone())
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        let result = serde_json::from_str::<RgbProxyMediaUploadRes>(&reponse)
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(result.clone())
    }

    pub async fn proxy_consig_retrieve(
        request_id: String,
    ) -> Result<Option<RgbProxyConsigRes>, ProxyServerError> {
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();

        let request_id = request_id.replace("utxob:", "");
        let url = format!("{endpoint}/proxy/consignment/{request_id}");
        let reponse = get(&url, None)
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        let resp = match serde_json::from_str::<RgbProxyConsigRes>(&reponse) {
            Ok(resp) => Some(resp),
            Err(_) => None,
        };

        Ok(resp)
    }

    pub async fn proxy_media_retrieve(
        attachment_id: String,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();

        let url = format!("{endpoint}/proxy/media/{attachment_id}");
        let reponse = get(&url, None)
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        let resp = match serde_json::from_str::<RgbProxyMediaRes>(&reponse) {
            Ok(resp) => Some(resp),
            Err(_) => None,
        };
        Ok(resp)
    }
}