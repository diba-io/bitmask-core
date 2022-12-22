#![cfg(feature = "server")]
use std::{env, net::SocketAddr, str::FromStr};

use anyhow::Result;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use bitmask_core::{
    accept_transfer, create_asset,
    data::structs::{
        AcceptRequest, AssetRequest, BlindRequest, DeclareRequest, IssueRequest, TransferRequest,
        TransferResponse,
    },
    get_blinded_utxo, import_asset, transfer_assets,
};
use log::info;
use rgb_std::Contract;
use tower_http::cors::CorsLayer;

async fn issue(Json(issue): Json<IssueRequest>) -> Result<impl IntoResponse, AppError> {
    let issue_res = create_asset(
        &issue.ticker,
        &issue.name,
        issue.precision,
        issue.supply,
        &issue.utxo,
    )?;

    Ok((StatusCode::OK, Json(issue_res)))
}

async fn blind(Json(blind): Json<BlindRequest>) -> Result<impl IntoResponse, AppError> {
    let blind_res = get_blinded_utxo(&blind.utxo)?;

    Ok((StatusCode::OK, Json(blind_res)))
}

async fn import(Json(asset): Json<AssetRequest>) -> Result<impl IntoResponse, AppError> {
    let asset_res = import_asset(&asset.asset, asset.utxos)?;

    Ok((StatusCode::OK, Json(asset_res)))
}

#[axum_macros::debug_handler]
async fn transfer(Json(transfer): Json<TransferRequest>) -> Result<impl IntoResponse, AppError> {
    let (consignment, psbt, disclosure, _, previous_utxo, _) = transfer_assets(
        &transfer.rgb_assets_descriptor_xpub,
        &transfer.blinded_utxo,
        transfer.amount,
        &transfer.asset_contract,
        transfer.asset_utxos,
    )
    .await?;

    let contract = Contract::from_str(&transfer.asset_contract)?;
    let transfer_res = TransferResponse {
        consignment,
        psbt,
        disclosure,
        declare_request: DeclareRequest {
            previous_utxo,
            asset_id: contract.contract_id().to_string(),
            new_outpoint: None,
            blinded_outpoint: None,
        },
    };

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn accept(Json(accept): Json<AcceptRequest>) -> Result<impl IntoResponse, AppError> {
    accept_transfer(
        &accept.consignment,
        &accept.blinding_factor,
        &accept.outpoint,
    )
    .await?;

    Ok(StatusCode::OK)
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug");
    }

    pretty_env_logger::init();

    let app = Router::new()
        .route("/issue", post(issue))
        .route("/blind", post(blind))
        .route("/import", post(import))
        .route("/transfer", post(transfer))
        .route("/accept", post(accept))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 7070));

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
