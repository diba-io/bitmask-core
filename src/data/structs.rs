use std::str::FromStr;

// Desktop
#[cfg(not(target_arch = "wasm32"))]
use bitcoin::psbt::PartiallySignedTransaction;
#[cfg(not(target_arch = "wasm32"))]
use rgb_core::validation::Status;
#[cfg(not(target_arch = "wasm32"))]
use rgb_core::value::Revealed;
#[cfg(not(target_arch = "wasm32"))]
use rgb_core::SealEndpoint;
#[cfg(not(target_arch = "wasm32"))]
use rgb_std::AssignedState;
#[cfg(not(target_arch = "wasm32"))]
use rgb_std::{Disclosure, InmemConsignment, TransferConsignment};

// Shared
use bdk::{Balance, BlockTime, LocalUtxo};
use bitcoin::{util::address::Address, OutPoint, Txid};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletData {
    pub address: String,
    pub balance: Balance,
    pub transactions: Vec<WalletTransaction>,
    pub utxos: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WalletTransaction {
    pub txid: Txid,
    pub received: u64,
    pub sent: u64,
    pub fee: Option<u64>,
    pub confirmed: bool,
    pub confirmation_time: Option<BlockTime>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedWalletData {
    pub btc_descriptor_xprv: String,
    pub btc_descriptor_xpub: String,
    pub btc_change_descriptor_xprv: String,
    pub btc_change_descriptor_xpub: String,
    pub rgb_assets_descriptor_xprv: String,
    pub rgb_assets_descriptor_xpub: String,
    pub rgb_udas_descriptor_xprv: String,
    pub rgb_udas_descriptor_xpub: String,
    pub xprvkh: String,
    pub xpubkh: String,
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FundVaultDetails {
    pub assets_output: Option<String>,
    pub assets_change_output: Option<String>,
    pub udas_output: Option<String>,
    pub udas_change_output: Option<String>,
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
    /// Name of the asset
    pub name: String,
    /// Description of the asset
    pub description: String,
    /// Precision of the asset
    pub precision: u8,
    /// Amount of the asset
    pub supply: u64,
    /// Seal of the initial owner
    pub seal: String,
    /// The name of the iface (ex: RGB20)
    pub iface: String,
}

#[derive(Serialize, Deserialize)]
pub struct IssueResult {
    pub genesis: String,   // in bech32m encoding
    pub id: String,        // contract ID
    pub asset_id: String,  // asset ID
    pub schema_id: String, // schema ID (i.e., RGB20)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InvoiceRequest {
    /// The contract id
    pub contract_id: String,
    /// The contract interface
    pub iface: String,
    /// Amount of the asset
    pub amount: u64,
    /// UTXO or Blinded UTXO
    pub seal: String,
}

#[derive(Serialize, Deserialize)]
pub struct InvoiceResult {
    /// Invoice encoded in Baid58
    pub invoice: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PsbtRequest {
    /// Descriptor XPub
    pub descriptor_pub: String,
    /// Asset UTXO
    pub asset_utxo: String,
    /// Asset UTXO Terminator
    pub asset_utxo_terminal: String,
    /// Asset Change Index UTXO
    pub change_index: Option<String>,
    /// Bitcoin Addresses (AddressFormat)
    pub bitcoin_changes: Vec<String>,
    /// Fee
    pub fee: u64,
}

#[derive(Serialize, Deserialize)]
pub struct PsbtResult {
    /// PSBT encoded in Base64
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RgbTransferRequest {
    /// RGB Invoice
    pub rgb_invoice: String,
    /// PSBT File Information
    pub psbt: String,
}

#[derive(Serialize, Deserialize)]
pub struct RgbTransferResult {
    /// Consignment encoded in baid58
    pub consig: String,
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
    pub asset: String,
    pub utxos: Vec<String>,
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

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display("{address}:{amount}", alt = "{address:#}:{amount:#}")]
pub struct AddressAmount {
    pub address: Address,
    pub amount: u64,
}

/// Error parsing representation
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AddressFormatParseError;

impl FromStr for AddressAmount {
    type Err = AddressFormatParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split: Vec<&str> = s.split(":").collect();
        let address = Address::from_str(split[0]).expect("");
        let amount = u64::from_str(split[1]).expect("");
        Ok(AddressAmount { address, amount })
    }
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
    pub genesis: String,
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

/// A blinded UTXO is an outpoint (txid:vout) that has an associated blinding factor to be kept track of separately.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindingUtxo {
    /// An encoded blinded UTXO. Sort of like an RGB address used to receive assets.
    /// Example: `"txob1gv9338jucjwledjqel62gg5nxy2kle5r2dk255ky3reevtjsx00q3nf3fe"`
    pub conceal: String,
    /// 64-bit blinding factor to reveal assets sent to the blinded UTXO. Helps with privacy.
    /// Example: `"8394351521931962961"`
    pub blinding: String,
    /// Outpoint struct
    pub utxo: OutPoint,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferRequestExt {
    pub inputs: Vec<OutPoint>,
    pub allocate: Vec<SealCoins>,
    pub receiver: String,
    pub amount: u64,
    pub asset: String,
    pub witness: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferResult {
    pub consignment: String,
    pub disclosure: String,
    pub txid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransfersRequest {
    pub descriptor_xpub: String, // TODO: Privacy concerns. Not great, not terrible
    pub transfers: Vec<AssetTransfer>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransfersResponse {
    pub psbt: PartiallySignedTransaction,
    pub origin: Vec<AssetUtxo>,
    pub disclosure: Disclosure,
    pub transfers: Vec<(InmemConsignment<TransferConsignment>, Vec<SealEndpoint>)>,
    pub transaction_info: Vec<AssetTransferInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransfersSerializeResponse {
    pub psbt: String,
    pub declare: DeclareRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValidateRequest {
    pub consignment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetTransfer {
    pub asset_contract: String,
    pub asset_utxo: AssetUtxo,
    pub asset_amount: u64,
    pub change_utxo: String,
    pub beneficiaries: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetTransferInfo {
    pub asset_contract: String,
    pub consignment: String,
    pub asset_utxo: String,
    pub change_utxo: String,
    pub change: u64,
    pub beneficiaries: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetUtxo {
    pub outpoint: String,
    pub terminal_derivation: String,
    pub commitment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AcceptRequest {
    pub consignment: String,
    pub blinding_factor: String,
    pub outpoint: String,
    pub blinded: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AcceptResponse {
    pub id: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub info: Status,
    pub valid: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AcceptLambdaResponse {
    pub accept: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlindedOrNotOutpoint {
    pub outpoint: String,
    pub balance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FinalizeTransfer {
    pub previous_utxo: String,
    pub consignment: String,
    pub asset: String,
    pub beneficiaries: Vec<BlindedOrNotOutpoint>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FullUtxo {
    pub utxo: LocalUtxo,
    pub terminal_derivation: String,
    pub commitment: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct FullCoin {
    pub coin: AssignedState<Revealed>,
    pub terminal_derivation: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeclareRequest {
    pub disclosure: String,
    pub change_transfers: Vec<ChangeTansfer>,
    pub transfers: Vec<FinalizeTransfer>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChangeTansfer {
    pub previous_utxo: String,
    pub asset: String,
    pub change: BlindedOrNotOutpoint,
}
