use bp::Outpoint;
use garde::Validate;
use rgb::MiningStatus;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use bdk::{Balance, BlockTime, TransactionDetails};
pub use bitcoin::{util::address::Address, Txid};
use rgbstd::interface::rgb21::Allocation as AllocationUDA;

use crate::validators::{
    verify_descriptor, verify_media_types, verify_rgb_invoice, verify_tapret_seal,
    verify_terminal_path, RGBContext,
};

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

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop, Display, Default)]
#[display(inner)]
pub struct SecretString(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct DecryptedWalletData {
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
pub struct FundVaultDetails {
    pub assets_output: Option<String>,
    pub assets_change_output: Option<String>,
    pub udas_output: Option<String>,
    pub udas_change_output: Option<String>,
    pub is_funded: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IssueAssetRequest {
    pub sk: String,
    pub request: IssueRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct IssueRequest {
    /// The ticker of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 8))]
    pub ticker: String,
    /// Name of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 40))]
    pub name: String,
    /// Description of the asset
    #[garde(ascii)]
    #[garde(length(min = 0, max = u8::MAX))]
    pub description: String,
    /// Amount of the asset
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub supply: u64,
    /// Precision of the asset
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub precision: u8,
    /// Seal of the initial owner
    #[garde(ascii)]
    #[garde(custom(verify_tapret_seal))]
    pub seal: String,
    /// The name of the iface (ex: RGB20)
    #[garde(alphanumeric)]
    pub iface: String,
    /// contract metadata (only RGB21/UDA)
    #[garde(custom(verify_media_types))]
    pub meta: Option<IssueMetaRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct SelfIssueRequest {
    /// The ticker of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 8))]
    pub ticker: String,
    /// Name of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 40))]
    pub name: String,
    /// Description of the asset
    #[garde(ascii)]
    #[garde(length(min = 0, max = u8::MAX))]
    pub description: String,
    /// contract metadata (only RGB21/UDA)
    #[garde(custom(verify_media_types))]
    pub meta: Option<IssueMetaRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct OverListContractsRequest {
    /// previous contracts
    #[garde(skip)]
    pub contracts: Vec<ContractResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct IssueMetaRequest(pub IssueMetadata);

impl IssueMetaRequest {
    pub fn with(metadata: IssueMetadata) -> Self {
        IssueMetaRequest(metadata)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum IssueMetadata {
    #[serde(rename = "uda")]
    UDA(Vec<MediaInfo>),

    #[serde(rename = "collectible")]
    Collectible(Vec<NewCollectible>),
}

impl Default for IssueMetadata {
    fn default() -> Self {
        IssueMetadata::UDA(vec![])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct NewCollectible {
    /// The ticker of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 8))]
    pub ticker: String,
    /// Name of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 40))]
    pub name: String,
    /// Description of the asset
    #[garde(ascii)]
    #[garde(length(min = 0, max = u8::MAX))]
    pub description: String,
    /// attachments and media
    #[garde(length(min = 1, max = u8::MAX))]
    pub media: Vec<MediaInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct MediaInfo {
    /// Mime Type of the media
    #[serde(rename = "type")]
    #[garde(ascii)]
    #[garde(length(min = 1, max = 64))]
    pub ty: String,
    /// Source (aka. hyperlink) of the media
    #[garde(ascii)]
    #[garde(length(min = 0, max = u16::MAX))]
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct AttachInfo {
    /// Mime Type of the media
    #[serde(rename = "type")]
    #[garde(ascii)]
    #[garde(length(min = 1, max = 64))]
    pub ty: String,
    /// Source (aka. hyperlink) of the media
    #[garde(ascii)]
    #[garde(length(min = 0, max = u16::MAX))]
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct IssueResponse {
    /// The contract id
    pub contract_id: String,
    /// The contract impl id
    pub iimpl_id: String,
    /// The contract interface
    pub iface: String,
    /// The Issue Close Method
    pub issue_method: String,
    /// The Issue Utxo
    pub issue_utxo: String,
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// creation date (timestamp)
    pub created: i64,
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
    /// contract metadata (only RGB21/UDA)
    pub meta: Option<ContractMeta>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContractFormats {
    /// The contract state (encoded in bech32m)
    pub legacy: String,
    /// The contract state (encoded in strict)
    pub strict: String,
    /// The contract state (compiled in armored mode)
    pub armored: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenesisFormats {
    /// The genesis state (encoded in bech32m)
    pub legacy: String,
    /// The genesis state (encoded in strict)
    pub strict: String,
    /// The contract state (compiled in armored mode)
    pub armored: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReIssueResponse {
    pub contracts: Vec<IssueResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    #[serde(rename = "bitcoin")]
    Bitcoin = 0,
    #[serde(rename = "change")]
    Change = 1,
    #[serde(rename = "contract")]
    Contract = 10,
    #[serde(rename = "rgb20")]
    RGB20 = 20,
    #[serde(rename = "rgb21")]
    RGB21 = 21,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct ImportRequest {
    /// The type data
    #[garde(skip)]
    pub import: AssetType,
    /// The payload data (in hexadecimal)
    #[garde(ascii)]
    #[garde(length(min = 0, max = U64))]
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractResponse {
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
    /// creation date (timestamp)
    pub created: i64,
    /// Description of the asset
    pub description: String,
    /// Is the contract hidden for the user?
    pub hidden: bool,
    /// Amount of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: u8,
    /// The user contract balance
    pub balance: u64,
    /// The contract allocations
    pub allocations: Vec<AllocationDetail>,
    /// The contract state (multiple formats)
    pub contract: ContractFormats,
    /// The genesis state (multiple formats)
    pub genesis: GenesisFormats,
    /// contract metadata (only RGB21/UDA)
    pub meta: Option<ContractMeta>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContractMeta(ContractMetadata);

impl ContractMeta {
    pub fn with(metadata: ContractMetadata) -> Self {
        ContractMeta(metadata)
    }

    pub fn meta(self) -> ContractMetadata {
        self.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ContractMetadata {
    #[serde(rename = "uda")]
    UDA(UDADetail),

    #[serde(rename = "collectible")]
    Collectible(Vec<UDADetail>),
}

impl Default for ContractMetadata {
    fn default() -> Self {
        ContractMetadata::UDA(UDADetail::default())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UDADetail {
    /// the token index of the uda
    pub token_index: u32,
    /// The ticker of the uda
    pub ticker: String,
    /// Name of the uda
    pub name: String,
    /// Description of the uda
    pub description: String,
    /// The user contract balance
    pub balance: u64,
    /// Media of the uda
    pub media: Vec<MediaInfo>,
    /// Attach of the uda
    pub attach: Option<AttachInfo>,
    /// The contract allocations
    pub allocations: Vec<AllocationDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct InvoiceRequest {
    /// The contract id
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// The contract interface
    #[garde(ascii)]
    #[garde(length(min = 0, max = 32))]
    pub iface: String,
    /// Amount of the asset
    #[garde(range(min = 0, max = u64::MAX))]
    pub amount: u64,
    /// Blinded UTXO
    #[garde(ascii)]
    #[garde(custom(verify_tapret_seal))]
    pub seal: String,
    /// Query parameters
    #[garde(skip)]
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct SelfInvoiceRequest {
    /// The contract id
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// Query parameters
    #[garde(skip)]
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceResponse {
    /// Invoice encoded in Baid58
    pub invoice: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct PsbtRequest {
    /// Asset UTXOs
    #[garde(dive)]
    #[garde(length(min = 0, max = 999))]
    pub asset_inputs: Vec<PsbtInputRequest>,
    /// Asset Descriptor Change
    #[garde(skip)]
    pub asset_descriptor_change: Option<SecretString>,
    /// Asset Terminal Change (default: /10/0)
    #[garde(skip)]
    pub asset_terminal_change: Option<String>,
    /// Bitcoin UTXOs
    #[garde(dive)]
    #[garde(length(min = 0, max = 999))]
    pub bitcoin_inputs: Vec<PsbtInputRequest>,
    /// Bitcoin Change Addresses (format: {address}:{amount})
    #[garde(length(min = 0, max = 999))]
    pub bitcoin_changes: Vec<String>,
    /// Bitcoin Fee
    #[garde(dive)]
    pub fee: PsbtFeeRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct PsbtInputRequest {
    /// Asset or Bitcoin Descriptor
    #[garde(custom(verify_descriptor))]
    pub descriptor: SecretString,
    /// Asset or Bitcoin UTXO
    #[garde(ascii)]
    pub utxo: String,
    /// Asset or Bitcoin UTXO Terminal (ex. /0/0)
    #[garde(custom(verify_terminal_path))]
    pub utxo_terminal: String,
    /// Asset or Bitcoin Tweak
    #[garde(skip)]
    pub tapret: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub enum PsbtFeeRequest {
    Value(#[garde(range(min = 0, max = u64::MAX))] u64),
    FeeRate(#[garde(skip)] f32),
}

impl Default for PsbtFeeRequest {
    fn default() -> Self {
        Self::Value(0)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]

pub struct PsbtResponse {
    /// PSBT encoded in Base64
    pub psbt: String,
    /// Asset UTXO Terminal (ex. /0/0)
    pub terminal: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct SignPsbtRequest {
    /// PSBT encoded in Base64
    #[garde(length(min = 0, max = u64::MAX))]
    pub psbt: String,
    /// Descriptors to Sign
    // TODO: Check secure way to validate xpriv desc
    #[garde(skip)]
    pub descriptors: Vec<SecretString>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignPsbtResponse {
    /// PSBT is signed?
    pub sign: bool,
    /// Transaction id
    pub txid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct RgbTransferRequest {
    /// RGB Invoice
    #[garde(ascii)]
    #[garde(length(min = 0, max = 512))]
    #[garde(custom(verify_rgb_invoice))]
    pub rgb_invoice: String,
    /// PSBT File Information
    #[garde(ascii)]
    #[garde(length(min = 0, max = U64))]
    pub psbt: String,
    /// Asset UTXO Terminal (ex. /0/0)
    #[garde(custom(verify_terminal_path))]
    pub terminal: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct FullRgbTransferRequest {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// The contract interface
    #[garde(ascii)]
    #[garde(length(min = 0, max = 32))]
    pub iface: String,
    /// RGB Invoice
    #[garde(ascii)]
    #[garde(length(min = 0, max = 512))]
    pub rgb_invoice: String,
    /// Asset Descriptor
    #[garde(custom(verify_descriptor))]
    pub descriptor: SecretString,
    /// Asset Terminal Change
    #[garde(ascii)]
    pub change_terminal: String,
    /// Bitcoin Fee
    #[garde(dive)]
    pub fee: PsbtFeeRequest,
    /// Bitcoin Change Addresses (format: {address}:{amount})
    #[garde(length(min = 0, max = 999))]
    pub bitcoin_changes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct SelfFullRgbTransferRequest {
    /// The contract id
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// The contract interface
    #[garde(ascii)]
    #[garde(length(min = 0, max = 32))]
    pub iface: String,
    /// RGB Invoice
    #[garde(ascii)]
    #[garde(length(min = 0, max = 512))]
    pub rgb_invoice: String,
    /// Bitcoin Change Terminal
    #[garde(ascii)]
    #[garde(length(min = 4, max = 4))]
    pub terminal: String,
    /// Bitcoin Change Addresses (format: {address}:{amount})
    #[garde(length(min = 0, max = 999))]
    pub bitcoin_changes: Vec<String>,
    /// Bitcoin Fee
    #[garde(skip)]
    pub fee: Option<u64>,
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
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct AcceptRequest {
    /// Consignment encoded in hexadecimal
    #[garde(ascii)]
    #[garde(length(min = 0, max = U64))]
    pub consignment: String,
    /// Force Consignment accept
    #[garde(skip)]
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
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct RgbSaveTransferRequest {
    /// The name of the iface (ex: RGB20)
    #[garde(alphanumeric)]
    pub iface: String,
    /// Consignment encoded in hexadecimal
    #[garde(ascii)]
    #[garde(length(min = 0, max = U64))]
    pub consignment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct RgbRemoveTransferRequest {
    /// Contract ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,

    /// Consignment ID
    #[garde(length(min = 1, max = 999))]
    pub consig_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferStatusResponse {
    /// Contract ID
    pub contract_id: String,
    /// Transfer ID
    pub consig_status: BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractsResponse {
    /// List of avaliable contracts
    pub contracts: Vec<ContractResponse>,
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
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct WatcherRequest {
    /// The watcher name
    #[garde(ascii)]
    #[garde(length(min = 1, max = 32))]
    pub name: String,
    /// The xpub will be watch
    #[garde(ascii)]
    #[garde(length(min = 1, max = 64))]
    pub xpub: String,
    /// Force recreate
    #[garde(skip)]
    pub force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherResponse {
    /// The watcher name
    pub name: String,
    /// migrate?
    pub migrate: bool,
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

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AllocationDetail {
    /// Anchored UTXO
    pub utxo: String,
    /// Asset Value
    pub value: AllocationValue,
    /// Derivation Path
    pub derivation: String,
    /// My Allocation?
    pub is_mine: bool,
    /// Allocation spent?
    pub is_spent: bool,
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Display)]
#[serde(rename_all = "camelCase")]
pub enum AllocationValue {
    #[display(inner)]
    #[serde(rename = "value")]
    Value(u64),
    #[serde(rename = "uda")]
    UDA(UDAPosition),
}

#[derive(
    Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Default, Display,
)]
#[serde(rename_all = "camelCase")]
#[display("{token_index}:{fraction}")]
pub struct UDAPosition {
    pub token_index: u32,
    pub fraction: u64,
}

impl UDAPosition {
    pub fn with(uda: AllocationUDA) -> Self {
        UDAPosition {
            token_index: uda
                .clone()
                .token_id()
                .to_string()
                .parse()
                .expect("invalid token_index"),
            fraction: uda
                .fraction()
                .to_string()
                .parse()
                .expect("invalid fraction"),
        }
    }
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
    pub utxo: Option<UtxoResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextUtxosResponse {
    pub utxos: Vec<UtxoResponse>,
}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone, Default, Display)]
#[serde(rename_all = "camelCase")]
#[display("{outpoint}:{amount}")]
pub struct UtxoResponse {
    pub outpoint: String,
    pub amount: u64,
    pub status: TxStatus,
}

impl UtxoResponse {
    pub fn with(outpoint: Outpoint, amount: u64, status: MiningStatus) -> Self {
        UtxoResponse {
            amount,
            outpoint: outpoint.to_string(),
            status: match status {
                MiningStatus::Mempool => TxStatus::Mempool,
                MiningStatus::Blockchain(h) => TxStatus::Block(h),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatcherUtxoResponse {
    pub utxos: Vec<String>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub filename: String,
    pub metadata: [u8; 8],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransfersResponse {
    /// List of avaliable transfers
    pub transfers: Vec<RgbTransferDetail>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferDetail {
    pub consig_id: String,
    pub status: TxStatus,
    #[serde(rename = "type")]
    pub ty: TransferType,
}

#[derive(
    Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Clone, Debug, Display, Default,
)]
#[serde(rename_all = "camelCase")]
pub enum TxStatus {
    #[default]
    #[display(inner)]
    #[serde(rename = "not_found")]
    NotFound,
    #[serde(rename = "error")]
    Error(String),
    #[serde(rename = "mempool")]
    Mempool,
    #[serde(rename = "block")]
    Block(u32),
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Clone, Debug, Display)]
#[serde(rename_all = "camelCase")]
pub enum TransferType {
    #[display(inner)]
    #[serde(rename = "sended")]
    Sended,
    #[serde(rename = "received")]
    Received,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbInvoiceResponse {
    pub contract_id: String,
    pub amount: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchRgbTransferResponse {
    pub transfers: Vec<BatchRgbTransferItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchRgbTransferItem {
    pub contract_id: String,
    pub consig_id: String,
    pub iface: String,
    pub status: TxStatus,
    pub is_accept: bool,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Display)]
#[display("{utxo}:{is_spent}")]
pub struct UtxoSpentStatus {
    pub utxo: String,
    pub is_spent: bool,
    pub block_height: TxStatus,
    pub spent_height: TxStatus,
}
