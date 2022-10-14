use crate::{
    data::constants::LNDHUB_ENDPOINT,
    util::{get, post_json},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Lightning wallet credentials
#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub login: String,
    pub password: String,
}

/// Lightning wallet tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct Tokens {
    pub refresh_token: String,
    pub access_token: String,
}

/// Add invoice request
#[derive(Debug, Serialize, Deserialize)]
pub struct AddInvoiceReq {
    pub memo: String,
    pub amt: String,
}

/// Add invoice response
#[derive(Debug, Serialize, Deserialize)]
pub struct AddInvoiceRes {
    pub payment_request: String,
}

/// User balance response
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceRes {
    #[serde(rename = "BTC")]
    pub btc: Balance,
}

/// User balance
#[derive(Debug, Serialize, Deserialize)]
pub struct Balance {
    #[serde(rename = "AvailableBalance")]
    pub available_balance: u64,
}

/// Lightning Invoice decoded
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Invoice {
    pub destination: String,
    pub payment_hash: String,
    pub num_satoshis: String,
    pub timestamp: String,
    pub expiry: String,
    pub description: String,
    pub description_hash: String,
    pub fallback_addr: String,
    pub cltv_expiry: String,
    pub route_hints: Vec<Hint>,
}

/// Pay invoice response
#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceRes {
    pub payment_preimage: String,
}

/// Pay invoice error
#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceError {
    pub error: bool,
    pub code: u8,
    pub message: String,
}

/// Lightning invoice hint
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Hint {
    pub node_id: String,
    pub chan_id: String,
    pub fee_base_msat: String,
    pub fee_proportional_millionths: String,
    pub cltv_expiry_delta: String,
}

/// Invoice request
#[derive(Debug, Serialize, Deserialize)]
pub struct InvoiceReq {
    pub invoice: String,
}

/// Payment request
#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub pay_req: String,
}

/// Creates a new lightning custodial wallet
pub async fn create_wallet() -> Result<Credentials> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let create_url = format!("{endpoint}/create");
    let (response, _) = post_json::<Credentials>(&create_url, &None, None).await?;
    let creds: Credentials = serde_json::from_str(&response)?;

    Ok(creds)
}

/// Get a auth tokens
pub async fn auth(creds: Credentials) -> Result<Tokens> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let auth_url = format!("{endpoint}/auth");
    let (response, _) = post_json(&auth_url, &Some(creds), None).await?;
    let tokens: Tokens = serde_json::from_str(&response)?;

    Ok(tokens)
}

/// Creates a lightning invoice
pub async fn create_invoice(description: &str, amount: u64, token: &str) -> Result<String> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/addinvoice");
    let req = AddInvoiceReq {
        memo: description.to_string(),
        amt: amount.to_string(),
    };
    let (response, _) = post_json(&url, &Some(req), Some(token)).await?;
    let invoice: AddInvoiceRes = serde_json::from_str(&response)?;

    Ok(invoice.payment_request)
}

/// Decode a lightning invoice (bolt11)
pub async fn decode_invoice(invoice: &str, token: &str) -> Result<Invoice> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/decodeinvoice?invoice={invoice}");
    let (response, _) = get(&url, Some(token)).await?;
    let invoice: Invoice = serde_json::from_str(&response)?;

    Ok(invoice)
}

/// Get user lightning balance
pub async fn get_balance(token: &str) -> Result<BalanceRes> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/balance");
    let (response, _) = get(&url, Some(token)).await?;
    let invoice: BalanceRes = serde_json::from_str(&response)?;

    Ok(invoice)
}

/// Pay a lightning invoice
pub async fn pay_invoice(invoice: &str, token: &str) -> Result<String> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/payinvoice");
    let req = InvoiceReq {
        invoice: invoice.to_string(),
    };
    let (response, _) = post_json(&url, &Some(req), Some(token)).await?;
    // match serde_json::from_str::<PayInvoiceRes>(&response) {
    //     Ok(response) => Ok(response),
    //     Err(e) => Err(e),
    // }
    // let r: PayInvoiceError = serde_json::from_str(&response)?;

    Ok(response)
}
