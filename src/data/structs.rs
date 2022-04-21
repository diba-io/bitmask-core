use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Issue {
    pub id: String,
    pub amount: u64,
    pub origin: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Amount {
    pub value: u64,
    pub blinding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub node_id: String,
    pub index: u32,
    pub outpoint: String,
    pub revealed_amount: Amount,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Inflation {}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub genesis: String,
    pub id: String,
    pub ticker: String,
    pub name: String,
    pub description: Option<String>,
    pub known_circulating: u64,
    pub is_issued_known: Option<String>,
    pub issue_limit: u64,
    pub chain: String,
    pub decimal_precision: u32,
    pub date: String,
    pub known_issues: Vec<Issue>,
    pub known_inflation: Inflation,
    pub known_allocations: Vec<Allocation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExportRequest {
    /// ContractId of the asset to export FROM the node
    pub asset: Option<String>,
    pub genesis: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExportRequestMini {
    /// ContractId of the asset to export FROM the node
    pub asset: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThinAsset {
    pub id: String,
    pub ticker: String,
    pub name: String,
    pub description: String,
    pub allocations: Vec<Allocation>,
    pub balance: Option<u64>,
    pub dolar_balance: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutPoint {
    pub txid: String,
    pub vout: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindResponse {
    pub blinding: String,
    pub conceal: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SealCoins {
    pub coins: u64,
    pub vout: u32,
    pub txid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferRequest {
    pub inputs: Vec<OutPoint>,
    pub allocate: Vec<SealCoins>,
    pub receiver: String,
    pub amount: u64,
    pub asset: String,
    pub witness: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferResponse {
    pub consignment: String,
    pub disclosure: String,
    pub witness: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValidateRequest {
    pub consignment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AcceptRequest {
    pub consignment: String,
    pub outpoint: OutPoint,
    pub blinding_factor: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncloseRequest {
    pub disclosure: String,
}
