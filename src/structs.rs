use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use bdk::{Balance, BlockTime, TransactionDetails};
pub use bitcoin::{util::address::Address, Txid};

use rgbstd::interface::rgb21::Allocation as AllocationUDA;

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

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
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
    /// contract metadata (only RGB21/UDA)
    pub meta: Option<IssueMetaRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SelfIssueRequest {
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// Description of the asset
    pub description: String,
    /// contract metadata (only RGB21/UDA)
    pub meta: Option<IssueMetaRequest>,
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
pub struct NewCollectible {
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// Description of the asset
    pub description: String,
    /// attachments and media
    pub media: Vec<MediaInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    /// Mime Type of the media
    #[serde(rename = "type")]
    pub ty: String,
    /// Source (aka. hyperlink) of the media
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    #[serde(rename = "bitcoin")]
    Bitcoin = 0,
    #[serde(rename = "contract")]
    Contract = 9,
    #[serde(rename = "rgb20")]
    RGB20 = 20,
    #[serde(rename = "rgb21")]
    RGB21 = 21,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// The type data
    pub import: AssetType,
    /// The payload data (in hexadecimal)
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
    /// Description of the asset
    pub description: String,
    /// Amount of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: u64,
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
    /// The contract allocations
    pub allocations: Vec<AllocationDetail>,
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
    /// Query parameters
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
pub struct PsbtRequest {
    /// Descriptor XPub
    pub descriptor_pub: String,
    /// Asset UTXO
    pub asset_utxo: String,
    /// Asset UTXO Terminal (ex. /0/0)
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
    /// Asset UTXO Terminal (ex. /0/0)
    pub terminal: String,
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
    /// iface
    pub iface: String,
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
pub struct RgbTransferRequest {
    /// RGB Invoice
    pub rgb_invoice: String,
    /// PSBT File Information
    pub psbt: String,
    /// Asset UTXO Terminal (ex. /0/0)
    pub terminal: String,
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
pub struct WatcherRequest {
    /// The watcher name
    pub name: String,
    /// The xpub will be watch
    pub xpub: String,
    /// Force recreate
    pub force: bool,
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
    pub value: AllocationValue,
    /// Derivation Path
    pub derivation: String,
    /// Derivation Path
    pub is_mine: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Display)]
#[serde(rename_all = "camelCase")]
pub enum AllocationValue {
    #[display(inner)]
    #[serde(rename = "value")]
    Value(u64),
    #[serde(rename = "uda")]
    UDA(UDAPosition),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Display)]
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
    pub utxo: String,
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
