use std::{env, net::SocketAddr};

use anyhow::Result;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use bitmask_core::{
    create_asset,
    data::structs::{AssetRequest, BlindRequest, IssueRequest, TransferRequest, ValidateRequest},
    import_asset, send_assets, set_blinded_utxo, validate_transaction,
};

async fn issue(Json(issue): Json<IssueRequest>) -> Result<impl IntoResponse, AppError> {
    let issue_res = create_asset(
        &issue.ticker,
        &issue.name,
        issue.precision,
        issue.supply,
        &issue.utxo.to_string(),
    )?;

    Ok((StatusCode::OK, Json(issue_res)))
}

async fn blind(Json(blind): Json<BlindRequest>) -> Result<impl IntoResponse, AppError> {
    let blind_res = set_blinded_utxo(&blind.utxo)?;

    Ok((StatusCode::OK, Json(blind_res)))
}

async fn import(Json(asset): Json<AssetRequest>) -> Result<impl IntoResponse, AppError> {
    let asset_res = import_asset(&asset.genesis)?;

    Ok((StatusCode::OK, Json(asset_res)))
}

async fn transfer(Json(transfer): Json<TransferRequest>) -> Result<impl IntoResponse, AppError> {
    let (_, _, transfer_res) = send_assets(
        "TODO: clientside transfer PSBT",
        "",
        "",
        transfer.amount,
        "",
    )
    .await?;

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn validate(Json(validate): Json<ValidateRequest>) -> Result<impl IntoResponse, AppError> {
    let asset_res = validate_transaction(&validate.consignment).await?;

    Ok((StatusCode::OK, Json(asset_res)))
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
        .route("/transfer", post(transfer)) // TODO: needs clientside transfer PSBT
        .route("/validate", post(validate));

    let addr = SocketAddr::from(([127, 0, 0, 1], 7070));
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
