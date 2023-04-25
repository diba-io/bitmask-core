// #![cfg(feature = "server")]
use std::{env, net::SocketAddr};

use anyhow::Result;
use axum::{
    body::Bytes,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bitcoin_hashes::hex::ToHex;
use bitmask_core::{
    accept_transfer, create_invoice, create_psbt,
    data::structs::{AcceptRequest, InvoiceRequest, IssueRequest, PsbtRequest, RgbTransferRequest},
    issue_contract, list_contracts, list_interfaces, list_schemas, pay_asset,
};
use carbonado::file::Header;
use log::info;
use tokio::fs;
use tower_http::cors::CorsLayer;

async fn issue(Json(issue): Json<IssueRequest>) -> Result<impl IntoResponse, AppError> {
    info!("POST /issue {issue:?}");

    let issue_res = issue_contract(
        &issue.ticker,
        &issue.name,
        &issue.description,
        issue.precision,
        issue.supply,
        &issue.seal,
        &issue.iface,
    )
    .await?;

    Ok((StatusCode::OK, Json(issue_res)))
}

async fn invoice(Json(invoice): Json<InvoiceRequest>) -> Result<impl IntoResponse, AppError> {
    info!("POST /invoice {invoice:?}");

    let invoice_res = create_invoice(
        &invoice.contract_id,
        &invoice.iface,
        invoice.amount,
        &invoice.seal,
    )
    .await?;

    Ok((StatusCode::OK, Json(invoice_res)))
}

async fn psbt(Json(psbt_req): Json<PsbtRequest>) -> Result<impl IntoResponse, AppError> {
    info!("POST /psbt {psbt_req:?}");

    let psbt_res = create_psbt(psbt_req).await?;

    Ok((StatusCode::OK, Json(psbt_res)))
}

#[axum_macros::debug_handler]
async fn pay(Json(pay_req): Json<RgbTransferRequest>) -> Result<impl IntoResponse, AppError> {
    info!("POST /pay {pay_req:?}");

    let transfer_res = pay_asset(pay_req).await?;

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn accept(Json(accept_req): Json<AcceptRequest>) -> Result<impl IntoResponse, AppError> {
    info!("POST /accept {accept_req:?}");

    let transfer_res = accept_transfer(accept_req).await?;

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn contracts() -> Result<impl IntoResponse, AppError> {
    info!("GET /contracts");

    let contracts_res = list_contracts().await?;

    Ok((StatusCode::OK, Json(contracts_res)))
}

async fn interfaces() -> Result<impl IntoResponse, AppError> {
    info!("GET /interfaces");

    let interfaces_res = list_interfaces().await?;

    Ok((StatusCode::OK, Json(interfaces_res)))
}

async fn schemas() -> Result<impl IntoResponse, AppError> {
    info!("GET /schemas");

    let schemas_res = list_schemas().await?;

    Ok((StatusCode::OK, Json(schemas_res)))
}

async fn co_store(body: Bytes) -> Result<impl IntoResponse, AppError> {
    info!("POST /carbonado {} bytes", body.len());

    let bytes = body.as_ref();
    let header: Header = Header::try_from(bytes)?;
    let pk = header.pubkey.to_hex();
    let path = "/tmp/bitmaskd/carbonado";
    let filename = format!("{path}/{pk}.c15");

    fs::create_dir_all(path).await?;
    info!("write {} bytes to {}", bytes.len(), filename);
    fs::write(filename, bytes).await?;

    Ok(StatusCode::OK)
}

async fn co_retrieve(Path(pk): Path<String>) -> Result<impl IntoResponse, AppError> {
    info!("GET /carbonado/{pk}");

    let path = "/tmp/bitmaskd/carbonado";
    let filename = format!("{path}/{pk}");
    info!("read {}", filename);
    let bytes = fs::read(filename).await?;

    Ok((StatusCode::OK, bytes))
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    pretty_env_logger::init();

    let app = Router::new()
        .route("/issue", post(issue))
        .route("/invoice", post(invoice))
        .route("/psbt", post(psbt))
        .route("/pay", post(pay))
        .route("/accept", post(accept))
        .route("/contracts", get(contracts))
        .route("/interfaces", get(interfaces))
        .route("/schemas", get(schemas))
        .route("/carbonado", post(co_store))
        .route("/carbonado/:pk", get(co_retrieve))
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
