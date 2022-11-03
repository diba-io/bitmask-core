use crate::{
    data::constants::LNDHUB_ENDPOINT,
    util::{get, post_json_auth},
};
use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};

/// Lightning wallet credentials
#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
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

/// Lightning invoice hint
#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
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

/// Lightning transaction
#[derive(Debug, Serialize, Deserialize)]
pub struct Tx {
    pub payment_preimage: Option<String>,
    pub payment_hash: Option<PaymentHash>,
    #[serde(rename = "type")]
    pub status: String,
    pub fee: u64,
    pub value: u64,
    #[serde(deserialize_with = "str_or_u64")]
    pub timestamp: u64,
    pub memo: String,
}

/// Payment hash
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentHash {
    #[serde(rename = "type")]
    pub _type: String,
    pub data: [u8; 32],
}

/// Pay invoice response
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PayInvoiceMessage {
    PayInvoiceResponse {
        payment_error: String,
        payment_preimage: PaymentHash,
        payment_route: Box<PaymentRoute>,
        payment_hash: PaymentHash,
        pay_req: String,
        decoded: Box<Invoice>,
    },
    PayInvoiceError {
        error: bool,
        code: u32,
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MppRecord {
    pub total_amt_msat: String,
    pub payment_addr: PaymentHash,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hop {
    pub chan_id: String,
    pub chan_capacity: String,
    pub amt_to_forward: String,
    pub fee: String,
    pub expiry: i64,
    pub amt_to_forward_msat: String,
    pub fee_msat: String,
    pub pub_key: String,
    pub tlv_payload: bool,
    pub mpp_record: MppRecord,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentRoute {
    pub hops: Vec<Hop>,
    pub total_time_lock: u32,
    pub total_fees: u64,
    pub total_amt: u64,
    pub total_fees_msat: u64,
    pub total_amt_msat: u64,
}

fn str_or_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrU64<'a> {
        Str(&'a str),
        U64(u64),
    }

    Ok(match StrOrU64::deserialize(deserializer)? {
        StrOrU64::Str(v) => v.parse().unwrap_or(0), // Ignoring parsing errors
        StrOrU64::U64(v) => v,
    })
}

/// Creates a new lightning custodial wallet
pub async fn create_wallet() -> Result<Credentials> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let create_url = format!("{endpoint}/create");
    let response = post_json_auth::<Credentials>(&create_url, &None, None).await?;
    let creds: Credentials = serde_json::from_str(&response)?;

    Ok(creds)
}

/// Get a auth tokens
pub async fn auth(creds: Credentials) -> Result<Tokens> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let auth_url = format!("{endpoint}/auth");
    let response = post_json_auth(&auth_url, &Some(creds), None).await?;
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
    let response = post_json_auth(&url, &Some(req), Some(token)).await?;
    let invoice: AddInvoiceRes = serde_json::from_str(&response)?;

    Ok(invoice.payment_request)
}

/// Decode a lightning invoice (bolt11)
pub async fn decode_invoice(invoice: &str, token: &str) -> Result<Invoice> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/decodeinvoice?invoice={invoice}");
    let response = get(&url, Some(token)).await?;
    let invoice: Invoice = serde_json::from_str(&response)?;

    Ok(invoice)
}

/// Get user lightning balance
pub async fn get_balance(token: &str) -> Result<BalanceRes> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/balance");
    let response = get(&url, Some(token)).await?;
    let invoice: BalanceRes = serde_json::from_str(&response)?;

    Ok(invoice)
}

/// Pay a lightning invoice
pub async fn pay_invoice(invoice: &str, token: &str) -> Result<PayInvoiceMessage> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/payinvoice");
    let req = InvoiceReq {
        invoice: invoice.to_string(),
    };
    let response = post_json_auth(&url, &Some(req), Some(token)).await?;
    let response: PayInvoiceMessage = serde_json::from_str(&response)?;

    Ok(response)
}

/// Get successful lightning transactions user made. Order newest to oldest.
pub async fn get_txs(token: &str, limit: u32, offset: u32) -> Result<Vec<Tx>> {
    let endpoint = LNDHUB_ENDPOINT.to_string();
    let url = format!("{endpoint}/gettxs?limit={}&offset={}", limit, offset);
    let response = get(&url, Some(token)).await?;
    let txs = serde_json::from_str(&response)?;

    Ok(txs)
}
