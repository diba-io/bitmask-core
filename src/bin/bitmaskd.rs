#![allow(unused_imports)]
#![cfg(feature = "server")]
#![cfg(not(target_arch = "wasm32"))]
use std::{env, fs::OpenOptions, io::ErrorKind, net::SocketAddr, str::FromStr};

use amplify::hex::ToHex;
use anyhow::Result;
use axum::{
    body::Bytes,
    extract::Path,
    headers::{authorization::Bearer, Authorization, CacheControl},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router, TypedHeader,
};
use bitcoin_30::secp256k1::{ecdh::SharedSecret, PublicKey, SecretKey};
use bitmask_core::{
    bitcoin::{decrypt_wallet, get_wallet_data, save_mnemonic, sign_psbt_file},
    carbonado::{handle_file, retrieve, retrieve_metadata},
    constants::{get_marketplace_seed, get_network, get_udas_utxo, switch_network},
    rgb::{
        accept_transfer, clear_watcher as rgb_clear_watcher, create_invoice, create_psbt,
        create_watcher, import as rgb_import, issue_contract, list_contracts, list_interfaces,
        list_schemas, reissue_contract, transfer_asset, watcher_address,
        watcher_details as rgb_watcher_details, watcher_next_address, watcher_next_utxo,
        watcher_utxo,
    },
    structs::{
        AcceptRequest, FileMetadata, ImportRequest, InvoiceRequest, IssueAssetRequest,
        IssueRequest, MediaInfo, PsbtRequest, ReIssueRequest, RgbTransferRequest, SecretString,
        SelfIssueRequest, SignPsbtRequest, WatcherRequest,
    },
};
use carbonado::file;
use log::{debug, error, info};
use rgbstd::interface::Iface;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tower_http::cors::CorsLayer;

async fn issue(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<IssueRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /issue {request:?}");

    let nostr_hex_sk = auth.token();
    let issue_res = issue_contract(nostr_hex_sk, request).await?;
    Ok((StatusCode::OK, Json(issue_res)))
}

async fn reissue(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<ReIssueRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /reissue {request:?}");

    let nostr_hex_sk = auth.token();
    let issue_res = reissue_contract(nostr_hex_sk, request).await?;
    Ok((StatusCode::OK, Json(issue_res)))
}

async fn self_issue(Json(issue): Json<SelfIssueRequest>) -> Result<impl IntoResponse, AppError> {
    info!("POST /self_issue {issue:?}");
    let issuer_keys = save_mnemonic(
        &SecretString(get_marketplace_seed().await),
        &SecretString("".to_string()),
    )
    .await?;

    let issue_seal = format!("tapret1st:{}", get_udas_utxo().await);
    let request = IssueRequest {
        ticker: issue.ticker,
        name: issue.name,
        description: issue.description,
        precision: 1,
        supply: 1,
        seal: issue_seal.to_owned(),
        iface: "RGB21".to_string(),
        meta: issue.meta,
    };

    let sk = issuer_keys.private.nostr_prv.as_ref();
    let issue_res = issue_contract(sk, request).await?;

    Ok((StatusCode::OK, Json(issue_res)))
}

async fn invoice(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(invoice): Json<InvoiceRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /invoice {invoice:?}");

    let nostr_hex_sk = auth.token();
    let invoice_res = create_invoice(nostr_hex_sk, invoice).await?;

    Ok((StatusCode::OK, Json(invoice_res)))
}

async fn _psbt(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(psbt_req): Json<PsbtRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /psbt {psbt_req:?}");

    let nostr_hex_sk = auth.token();

    let psbt_res = create_psbt(nostr_hex_sk, psbt_req).await?;

    Ok((StatusCode::OK, Json(psbt_res)))
}

async fn _sign_psbt(
    TypedHeader(_auth): TypedHeader<Authorization<Bearer>>,
    Json(psbt_req): Json<SignPsbtRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /sign {psbt_req:?}");
    let psbt_res = sign_psbt_file(psbt_req).await?;

    Ok((StatusCode::OK, Json(psbt_res)))
}

#[axum_macros::debug_handler]
async fn pay(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(pay_req): Json<RgbTransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /pay {pay_req:?}");

    let nostr_hex_sk = auth.token();

    let transfer_res = transfer_asset(nostr_hex_sk, pay_req).await?;

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn accept(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(accept_req): Json<AcceptRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /accept {accept_req:?}");

    let nostr_hex_sk = auth.token();
    let transfer_res = accept_transfer(nostr_hex_sk, accept_req).await?;

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn contracts(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /contracts");

    let nostr_hex_sk = auth.token();

    let contracts_res = list_contracts(nostr_hex_sk).await?;

    Ok((StatusCode::OK, Json(contracts_res)))
}

async fn contract_detail(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /contract/{name:?}");

    let nostr_hex_sk = auth.token();

    let contracts_res = list_contracts(nostr_hex_sk).await?;

    Ok((StatusCode::OK, Json(contracts_res)))
}

async fn interfaces(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /interfaces");

    let nostr_hex_sk = auth.token();

    let interfaces_res = list_interfaces(nostr_hex_sk).await?;

    Ok((StatusCode::OK, Json(interfaces_res)))
}

async fn schemas(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /schemas");

    let nostr_hex_sk = auth.token();

    let schemas_res = list_schemas(nostr_hex_sk).await?;

    Ok((StatusCode::OK, Json(schemas_res)))
}

async fn import(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(import_req): Json<ImportRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /import {import_req:?}");

    let nostr_hex_sk = auth.token();
    let import_res = rgb_import(nostr_hex_sk, import_req).await?;

    Ok((StatusCode::OK, Json(import_res)))
}

async fn watcher(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<WatcherRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /watcher {request:?}");

    let nostr_hex_sk = auth.token();
    let resp = create_watcher(nostr_hex_sk, request).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn watcher_details(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /watcher/{name:?}");

    let nostr_hex_sk = auth.token();
    let resp = rgb_watcher_details(nostr_hex_sk, &name).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn clear_watcher(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("DELETE /watcher/{name:?}");

    let nostr_hex_sk = auth.token();
    let resp = rgb_clear_watcher(nostr_hex_sk, &name).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn next_address(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path((name, asset)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /watcher/{name:?}/address");
    info!("GET /watcher/{name:?}/{asset:?}/address");

    let nostr_hex_sk = auth.token();
    let resp = watcher_next_address(nostr_hex_sk, &name, &asset).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn next_utxo(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path((name, asset)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /watcher/{name:?}/{asset:?}/utxo");

    let nostr_hex_sk = auth.token();
    let resp = watcher_next_utxo(nostr_hex_sk, &name, &asset).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn register_address(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path((name, address)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("PUT /watcher/{name:?}/address/{address:?}");

    let nostr_hex_sk = auth.token();
    let resp = watcher_address(nostr_hex_sk, &name, &address).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn register_utxo(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path((name, utxo)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("PUT /watcher/{name:?}/utxo/{utxo:?}");

    let nostr_hex_sk = auth.token();
    let resp = watcher_utxo(nostr_hex_sk, &name, &utxo).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn co_store(
    Path((pk, name)): Path<(String, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let incoming_header = carbonado::file::Header::try_from(&body)?;
    let body_len = incoming_header.encoded_len - incoming_header.padding_len;
    info!("POST /carbonado/{pk}/{name}, {body_len} bytes");

    let filepath = handle_file(&pk, &name, body_len.try_into()?).await?;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&filepath)
    {
        Ok(file) => {
            let present_header = match carbonado::file::Header::try_from(&file) {
                Ok(header) => header,
                _ => carbonado::file::Header::try_from(&body)?,
            };
            let present_len = present_header.encoded_len - present_header.padding_len;
            debug!("body len: {body_len} present_len: {present_len}");
            if body_len >= present_len {
                debug!("body is bigger, overwriting.");
                let resp = fs::write(&filepath, &body).await;
                debug!("write file status {}", resp.is_ok());
            } else {
                debug!("no file written.");
            }
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                debug!("no file found, writing {body_len} bytes.");
                fs::write(&filepath, &body).await?;
            }
            _ => {
                error!("error in POST /carbonado/{pk}/{name}: {err}");
                return Err(err.into());
            }
        },
    }

    let cc = CacheControl::new().with_no_cache();

    Ok((StatusCode::OK, TypedHeader(cc)))
}

async fn co_retrieve(
    Path((pk, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /carbonado/{pk}/{name}");

    let filepath = &handle_file(&pk, &name, 0).await?;
    let fullpath = filepath.to_string_lossy();
    let bytes = fs::read(filepath).await;
    let cc = CacheControl::new().with_no_cache();

    match bytes {
        Ok(bytes) => {
            debug!("read {0} bytes.", bytes.len());
            Ok((StatusCode::OK, TypedHeader(cc), bytes))
        }
        Err(e) => {
            debug!(
                "file read error {0} .Details: {1}.",
                fullpath,
                e.to_string()
            );
            Ok((StatusCode::OK, TypedHeader(cc), Vec::<u8>::new()))
        }
    }
}

async fn co_metadata(
    Path((pk, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /carbonado/{pk}/{name}/metadata");

    let filepath = &handle_file(&pk, &name, 0).await?;
    let mut metadata = FileMetadata::default();
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath)
    {
        Ok(file) => {
            let present_header = match carbonado::file::Header::try_from(&file) {
                Ok(header) => header,
                _ => return Ok((StatusCode::OK, Json(metadata))),
            };

            metadata.filename = present_header.file_name();
            metadata.metadata = present_header.metadata.unwrap_or_default();
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                fs::write(&filepath, &vec![]).await?;
            }
            _ => {
                error!("error in POST /carbonado/{pk}/{name}: {err}");
                return Err(err.into());
            }
        },
    }

    Ok((StatusCode::OK, Json(metadata)))
}

async fn status() -> Result<impl IntoResponse, AppError> {
    let cc = CacheControl::new().with_no_cache();

    Ok((StatusCode::OK, TypedHeader(cc), "OK".to_string()))
}

async fn key(Path(pk): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sk = env::var("NOSTR_SK")?;
    let sk = SecretKey::from_str(&sk)?;

    let pk = PublicKey::from_str(&pk)?;

    let ss = SharedSecret::new(&pk, &sk);
    let ss = ss.display_secret();

    Ok(ss.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "bitmask_core=debug,bitmaskd=debug");
    }

    pretty_env_logger::init();

    let app = Router::new()
        .route("/issue", post(issue))
        .route("/reissue", post(reissue))
        .route("/selfissue", post(self_issue))
        .route("/invoice", post(invoice))
        // .route("/psbt", post(psbt))
        // .route("/sign", post(sign_psbt))
        .route("/pay", post(pay))
        .route("/accept", post(accept))
        .route("/contracts", get(contracts))
        .route("/contract/:id", get(contract_detail))
        .route("/interfaces", get(interfaces))
        .route("/schemas", get(schemas))
        .route("/import", post(import))
        .route("/watcher", post(watcher))
        .route("/watcher/:name", get(watcher_details))
        .route("/watcher/:name/:asset/address", get(next_address))
        .route("/watcher/:name/:asset/utxo", get(next_utxo))
        .route(
            "/watcher/:name/:asset/address/:address",
            put(register_address),
        )
        .route("/watcher/:name/:asset/utxo/:utxo", put(register_utxo))
        .route("/watcher/:name", delete(clear_watcher))
        .route("/key/:pk", get(key))
        .route("/carbonado/status", get(status))
        .route("/carbonado/:pk/:name", post(co_store))
        .route("/carbonado/:pk/:name", get(co_retrieve))
        .route("/carbonado/:pk/:name/metadata", get(co_metadata))
        .layer(CorsLayer::permissive());

    let network = get_network().await;
    switch_network(&network).await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], 7070));

    info!("bitmaskd REST server successfully running at {addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

// https://github.com/tokio-rs/axum/blob/fef95bf37a138cdf94985e17f27fd36481525171/examples/anyhow-error-response/src/main.rs
// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
