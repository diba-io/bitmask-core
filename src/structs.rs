use serde::{Deserialize, Serialize};

pub use bdk::{Balance, BlockTime, TransactionDetails};
pub use bitcoin::{util::address::Address, Txid};

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
pub struct PrivateWalletData {
    pub xprvkh: String,
    pub btc_descriptor_xprv: String,
    pub btc_change_descriptor_xprv: String,
    pub rgb_assets_descriptor_xprv: String,
    pub rgb_udas_descriptor_xprv: String,
    pub nostr_prv: String,
    pub nostr_nsec: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublicWalletData {
    pub xpub: String,
    pub xpubkh: String,
    pub watcher_xpub: String,
    pub btc_descriptor_xpub: String,
    pub btc_change_descriptor_xpub: String,
    pub rgb_assets_descriptor_xpub: String,
    pub rgb_udas_descriptor_xpub: String,
    pub nostr_pub: String,
    pub nostr_npub: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedWalletData {
    pub mnemonic: String,
    pub private: PrivateWalletData,
    pub public: PublicWalletData,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedWalletDataV04 {
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
#[serde(rename_all = "camelCase")]
pub struct MnemonicSeedData {
    pub mnemonic: String,
    pub encrypted_descriptors: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FundVaultDetails {
    pub assets_output: Option<String>,
    pub assets_change_output: Option<String>,
    pub udas_output: Option<String>,
    pub udas_change_output: Option<String>,
    pub is_funded: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IssueRequest {
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// Description of the asset
    pub description: String,
    /// Amount of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: u8,
    /// Seal of the initial owner
    pub seal: String,
    /// The name of the iface (ex: RGB20)
    pub iface: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IssueResponse {
    /// The contract id
    pub contract_id: String,
    /// The contract impl id
    pub iimpl_id: String,
    /// The contract interface
    pub iface: String,
    /// The Issue Utxo
    pub issue_utxo: String,
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// Description of the asset
    pub description: String,
    /// Amount of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: u8,
    /// The contract state (multiple formats)
    pub contract: ContractFormats,
    /// The gensis state (multiple formats)
    pub genesis: GenesisFormats,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractFormats {
    /// The contract state (encoded in bech32m)
    pub legacy: String,
    /// The contract state (encoded in strict)
    pub strict: String,
    /// The contract state (compiled in armored mode)
    pub armored: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenesisFormats {
    /// The genesis state (encoded in bech32m)
    pub legacy: String,
    /// The genesis state (encoded in strict)
    pub strict: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ImportType {
    Contract,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// The type data
    pub import: ImportType,
    /// The payload data (in hexadecimal)
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportResponse {
    /// The contract id
    pub contract_id: String,
    /// The contract impl id
    pub iimpl_id: String,
    /// The contract interface
    pub iface: String,
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// Description of the asset
    pub description: String,
    /// Amount of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: String,
    /// The user contract balance
    pub balance: u64,
    /// The contract allocations
    pub allocations: Vec<AllocationDetail>,
    /// The contract state (multiple formats)
    pub contract: ContractFormats,
    /// The genesis state (multiple formats)
    pub genesis: GenesisFormats,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceResponse {
    /// Invoice encoded in Baid58
    pub invoice: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PsbtRequest {
    /// Descriptor XPub
    pub descriptor_pub: String,
    /// Asset UTXO
    pub asset_utxo: String,
    /// Asset UTXO Terminator (ex. /0/0)
    pub asset_utxo_terminal: String,
    /// Asset Change Index UTXO (default: 1)
    pub change_index: Option<u16>,
    /// Bitcoin Change Addresses (format: {address}:{amount})
    pub bitcoin_changes: Vec<String>,
    /// Bitcoin Fee
    pub fee: Option<u64>,
    /// TapTweak used to spend outputs based in tapret commitments
    pub input_tweak: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PsbtResponse {
    /// PSBT encoded in Base64
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignPsbtRequest {
    /// PSBT encoded in Base64
    pub psbt: String,
    /// mnemonic
    pub mnemonic: String,
    /// password
    pub seed_password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignPsbtResponse {
    /// PSBT is signed?
    pub sign: bool,
    /// Transaction encoded in Base64
    pub tx: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferRequest {
    /// RGB Invoice
    pub rgb_invoice: String,
    /// PSBT File Information
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferResponse {
    /// Consignment ID
    pub consig_id: String,
    /// Consignment encoded (in hexadecimal)
    pub consig: String,
    /// PSBT File Information with tapret (in hexadecimal)
    pub psbt: String,
    /// Tapret Commitment (used to spend output)
    pub commit: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AcceptRequest {
    /// Consignment encoded in hexadecimal
    pub consignment: String,
    /// Force Consignment accept
    pub force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AcceptResponse {
    /// Transfer ID
    pub transfer_id: String,
    /// Contract ID
    pub contract_id: String,
    /// Transfer accept status
    pub valid: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractResponse {
    /// Contract
    pub contract: ImportResponse,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractsResponse {
    /// List of avaliable contracts
    pub contracts: Vec<ImportResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfacesResponse {
    /// List of avaliable interfaces and implementations
    pub interfaces: Vec<InterfaceDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceDetail {
    /// Interface Name
    pub name: String,
    /// Interface ID
    pub iface: String,
    /// Interface ID
    pub iimpl: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SchemasResponse {
    /// List of avaliable schemas
    pub schemas: Vec<SchemaDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDetail {
    /// Schema ID
    pub schema: String,
    /// Avaliable Interfaces
    pub ifaces: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherRequest {
    /// The watcher name
    pub name: String,
    /// The xpub will be watch
    pub xpub: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherResponse {
    /// The watcher name
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatcherDetailResponse {
    /// Allocations
    pub contracts: Vec<WatcherDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherDetail {
    /// Contract ID
    pub contract_id: String,
    /// Allocations
    pub allocations: Vec<AllocationDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AllocationDetail {
    /// Anchored UTXO
    pub utxo: String,
    /// Asset Value
    pub value: u64,
    /// Derivation Path
    pub derivation: String,
    /// Derivation Path
    pub is_mine: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextAddressResponse {
    pub address: String,
    pub network: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextUtxoResponse {
    pub utxo: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SatsInvoice {
    pub amount: u64,
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// ContractId of the asset to export FROM the node
    pub asset: Option<String>,
    pub genesis: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequestMini {
    /// ContractId of the asset to export FROM the node
    pub asset: String,
}
