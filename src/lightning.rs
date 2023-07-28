use std::{collections::HashMap, str::FromStr};

use anyhow::Result;
use lightning_invoice::Invoice;
use serde::{Deserialize, Serialize};

use crate::{
    constants::LNDHUB_ENDPOINT,
    util::{get, post_json_auth},
};

/// Lightning wallet credentials
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Wallet creation response
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum CreateWalletResponse {
    Username { username: String },
    Error { error: String },
}

/// Auth response
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AuthResponse {
    Result { refresh: String, token: String },
    Error { error: String },
}

/// Amount and currency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Money {
    pub value: String,
    pub currency: String,
}

/// Add Invoice response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddInvoiceResponse {
    pub req_id: String,
    pub uid: u32,
    pub payment_request: Option<String>,
    pub meta: Option<String>,
    pub metadata: Option<String>,
    pub amount: Money,
    pub rate: Option<String>,
    pub currency: String,
    pub target_account_currency: Option<String>,
    pub account_id: Option<String>,
    pub error: Option<String>,
    pub fees: Option<String>,
}

/// User balance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancesResponse {
    pub uid: u32,
    pub accounts: HashMap<String, Account>,
    pub error: Option<String>,
}

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_id: String,
    pub balance: String,
    pub currency: String,
}

/// Pay Invoice request
#[derive(Debug, Serialize, Deserialize)]
pub struct PayInvoiceRequest {
    pub payment_request: String,
}

/// Lightning transaction
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub txid: String,
    pub fee_txid: Option<String>,
    pub outbound_txid: Option<String>,
    pub inbound_txid: Option<String>,
    pub created_at: u64,
    pub outbound_amount: String,
    pub inbound_amount: String,
    pub outbound_account_id: String,
    pub inbound_account_id: String,
    pub outbound_uid: u32,
    pub inbound_uid: u32,
    pub outbound_currency: String,
    pub inbound_currency: String,
    pub exchange_rate: String,
    pub tx_type: String,
    pub fees: String,
    pub reference: Option<String>,
}

/// Pay invoice response
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayInvoiceResponse {
    pub payment_hash: String,
    pub uid: u32,
    pub success: bool,
    pub currency: String,
    pub payment_request: Option<String>,
    pub amount: Option<Money>,
    pub fees: Option<Money>,
    pub error: Option<String>,
    pub payment_preimage: Option<String>,
    pub destination: Option<String>,
    pub description: Option<String>,
}

/// Check payment response
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckPaymentResponse {
    paid: bool,
}

/// Swap BTC onchain to Lightning response
#[derive(Debug, Serialize, Deserialize)]
pub struct SwapBtcLnResponse {
    pub address: String,
    pub commitment: String,
    pub signature: String,
    pub secret_access_key: String,
}

/// Speed of the onchain transaction
#[derive(Serialize, Deserialize, Debug)]
pub enum OnchainSpeed {
    Fast,
    Medium,
    Slow,
}

/// Swap Lightning to BTC onchain request
#[derive(Serialize, Deserialize, Debug)]
pub struct SwapLnBTCRequest {
    pub amount: u64,
    pub address: String,
    pub speed: Option<OnchainSpeed>,
}

/// Swap Lightning to BTC onchain response
#[derive(Debug, Serialize, Deserialize)]
pub struct SwapLnBtcResponse {
    pub bolt11_invoice: String,
    pub fee_sats: u32,
}

/// Creates a new lightning custodial wallet
pub async fn create_wallet(username: &str, password: &str) -> Result<CreateWalletResponse> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let creds = Credentials {
        username: username.to_string(),
        password: password.to_string(),
    };
    let create_url = format!("{endpoint}/create");
    let response = post_json_auth(&create_url, &Some(creds), None).await?;

    let res: CreateWalletResponse = serde_json::from_str(&response)?;

    Ok(res)
}

/// Get a auth tokens
pub async fn auth(username: &str, password: &str) -> Result<AuthResponse> {
    let creds = Credentials {
        username: username.to_string(),
        password: password.to_string(),
    };
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let auth_url = format!("{endpoint}/auth");
    let response = post_json_auth(&auth_url, &Some(creds), None).await?;
    let response: AuthResponse = serde_json::from_str(&response)?;

    Ok(response)
}

/// Creates a lightning invoice
pub async fn create_invoice(
    description: &str,
    amount: u32,
    token: &str,
) -> Result<AddInvoiceResponse> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let amount = amount as f32 / 100_000_000.0;
    let amt_str = amount.to_string();
    let url = format!("{endpoint}/addinvoice?amount={amt_str}&meta={description}");
    let response = get(&url, Some(token)).await?;
    let invoice: AddInvoiceResponse = serde_json::from_str(&response)?;

    Ok(invoice)
}

/// Decode a lightning invoice (bolt11)
pub fn decode_invoice(payment_request: &str) -> Result<Invoice> {
    let invoice = Invoice::from_str(payment_request)?;

    Ok(invoice)
}

/// Get user lightning balance
pub async fn get_balance(token: &str) -> Result<Vec<Account>> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let url = format!("{endpoint}/balance");
    let response = get(&url, Some(token)).await?;
    let balance: BalancesResponse = serde_json::from_str(&response)?;
    let mut accounts = Vec::new();
    for (_, value) in balance.accounts {
        accounts.push(value);
    }

    Ok(accounts)
}

/// Pay a lightning invoice
pub async fn pay_invoice(payment_request: &str, token: &str) -> Result<PayInvoiceResponse> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let url = format!("{endpoint}/payinvoice");
    let req = PayInvoiceRequest {
        payment_request: payment_request.to_string(),
    };
    let response = post_json_auth(&url, &Some(req), Some(token)).await?;
    let response: PayInvoiceResponse = serde_json::from_str(&response)?;

    Ok(response)
}

/// Get successful lightning transactions user made. Order newest to oldest.
pub async fn get_txs(token: &str) -> Result<Vec<Transaction>> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let url = format!("{endpoint}/gettxs");
    let response = get(&url, Some(token)).await?;
    let txs = serde_json::from_str(&response)?;

    Ok(txs)
}

/// Check if a lightning invoice has been paid
pub async fn check_payment(payment_hash: &str) -> Result<bool> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let url = format!("{endpoint}/checkpayment?payment_hash={payment_hash}");
    let response = get(&url, None).await?;
    let r = serde_json::from_str::<CheckPaymentResponse>(&response)?;

    Ok(r.paid)
}

/// Swap BTC onchain to Lightning
pub async fn swap_btc_ln(token: &str, ln_address: Option<String>) -> Result<SwapBtcLnResponse> {
    let ln_address_query = match ln_address {
        Some(a) => format!("?lnurl_or_lnaddress={}", a),
        None => "".to_string(),
    };
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let url = format!("{endpoint}/get_onchain_address{ln_address_query}");
    let response = get(&url, Some(token)).await?;
    let r = serde_json::from_str::<SwapBtcLnResponse>(&response)?;

    Ok(r)
}

/// Swap Lightning to BTC onchain
pub async fn swap_ln_btc(address: &str, amount: u64, token: &str) -> Result<SwapLnBtcResponse> {
    let endpoint = LNDHUB_ENDPOINT.read().await;
    let url = format!("{endpoint}/make_onchain_swap");
    let req = SwapLnBTCRequest {
        address: address.to_string(),
        amount,
        speed: Some(OnchainSpeed::Fast),
    };
    let response = post_json_auth(&url, &Some(req), Some(token)).await?;
    let r = serde_json::from_str::<SwapLnBtcResponse>(&response)?;

    Ok(r)
}
