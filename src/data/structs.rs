use bitcoin::{util::address::Address, OutPoint, Txid};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VaultData {
    pub btc_descriptor_xprv: String,
    pub btc_descriptor_xpub: String,
    pub btc_change_descriptor_xprv: String,
    pub btc_change_descriptor_xpub: String,
    pub rgb_assets_descriptor_xprv: String,
    pub rgb_assets_descriptor_xpub: String,
    pub rgb_udas_descriptor_xprv: String,
    pub rgb_udas_descriptor_xpub: String,
    pub xpubkh: String,
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FundVaultDetails {
    pub txid: String,
    pub assets: String,
    pub assets_change: String,
    pub udas: String,
    pub udas_change: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Issue {
    pub id: String,
    pub amount: u64,
    pub origin: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IssueRequest {
    /// The ticker of the asset
    pub ticker: String,
    /// The name of the asset
    pub name: String,
    /// Description of the asset (ID for the UDA)
    pub description: String,
    /// Precision of the asset
    pub precision: u8,
    /// Amount of the asset
    pub supply: u64,
    /// Utxo of the initial owner
    pub utxo: OutPoint,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SatsInvoice {
    pub amount: u64,
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Amount {
    pub value: u64,
    pub blinding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub index: u32,
    pub node_id: String,
    pub outpoint: String,
    pub amount: Amount,
    pub seal_vout: u32,
    pub seal_txid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Inflation {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetRequest {
    pub genesis: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetResponse {
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
    pub balance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindRequest {
    pub utxo: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindResponse {
    pub blinding: String,
    pub conceal: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SealCoins {
    pub amount: u64,
    pub txid: Txid,
    pub vout: u32,
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
    pub txid: String,
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
pub struct EncloseForgetRequest {
    pub outpoints: Vec<OutPoint>,
    pub disclosure: String,
}
