use bp::Outpoint;
use garde::Validate;
use psbt::Psbt;
use rgb::MiningStatus;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use bdk::{Balance, BlockTime, TransactionDetails};
pub use bitcoin::{util::address::Address, Txid};
use rgbstd::interface::rgb21::Allocation as AllocationUDA;

use crate::{
    rgb::swap::{PublicRgbBid, RgbBid, RgbOffer, RgbOfferSwap},
    validators::{
        verify_descriptor, verify_media_types, verify_rgb_invoice, verify_tapret_seal,
        verify_terminal_path, RGBContext,
    },
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
    pub udas_output: Option<String>,
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
    #[garde(length(min = u8::MIN.into(), max = u8::MAX.into()))]
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
    #[garde(length(min = u8::MIN.into(), max = u8::MAX.into()))]
    pub description: String,
    /// contract metadata (only RGB21/UDA)
    #[garde(custom(verify_media_types))]
    pub meta: Option<IssueMetaRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct ReIssueRequest {
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
    #[garde(length(min = u8::MIN.into(), max = u8::MAX.into()))]
    pub description: String,
    /// attachments and media
    #[garde(length(min = u8::MIN.into(), max = u8::MAX.into()))]
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
    #[garde(length(min = u16::MIN.into(), max = u16::MAX.into()))]
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
    #[garde(length(min = u16::MIN.into(), max = u16::MAX.into()))]
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

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
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
    #[garde(length(min = 0, max = usize::MAX))]
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractHiddenResponse {
    /// The contract id
    pub contract_id: String,
    /// is hidden
    pub hidden: bool,
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
    /// Supply of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: u8,
    /// Current balance
    pub balance: u64,
    /// Current balance (Humanized)
    pub balance_normalised: f64,
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

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display, Serialize, Deserialize)]
#[display("{contract_id}:{iface_id}:{precision}")]
pub struct SimpleContractResponse {
    pub contract_id: String,
    pub iface_id: String,
    pub precision: u8,
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
    #[garde(skip)]
    pub amount: String,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
    /// Allow RBF
    #[garde(skip)]
    pub rbf: bool,
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
    /// Asset or Bitcoin Tweak
    #[garde(skip)]
    pub sigh_hash: Option<PsbtSigHashRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum PsbtSigHashRequest {
    /// 0x1: Sign all outputs.
    #[default]
    All = 0x01,
    /// 0x2: Sign no outputs --- anyone can choose the destination.
    None = 0x02,
    /// 0x3: Sign the output whose index matches this input's index. If none exists,
    /// sign the hash `0000000000000000000000000000000000000000000000000000000000000001`.
    /// (This rule is probably an unintentional C++ism, but it's consensus so we have
    /// to follow it.)
    Single = 0x03,
    /// 0x81: Sign all outputs but only this input.
    AllPlusAnyoneCanPay = 0x81,
    /// 0x82: Sign no outputs and only this input.
    NonePlusAnyoneCanPay = 0x82,
    /// 0x83: Sign one output and only this input (see `Single` for what "one output" means).
    SinglePlusAnyoneCanPay = 0x83,
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
    #[garde(length(min = 0, max = usize::MAX))]
    pub psbt: String,
    /// Descriptors to Sign
    // TODO: Check secure way to validate xpriv desc
    #[garde(skip)]
    pub descriptors: Vec<SecretString>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignedPsbtResponse {
    /// PSBT is signed?
    pub sign: bool,
    /// PSBT signed
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct PublishPsbtRequest {
    /// PSBT encoded in Base64
    #[garde(length(min = 0, max = usize::MAX))]
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPsbtResponse {
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
    #[garde(length(min = 0, max = usize::MAX))]
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct RgbInternalSaveTransferRequest {
    /// The Consignment ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub consig_id: String,
    /// The Face Symbol
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub iface: String,
    /// Consignment Data (Hex)
    #[garde(ascii)]
    pub consig: String,
    /// Beneficiary
    #[garde(ascii)]
    pub beneficiary: String,
    /// Sender?
    #[garde(skip)]
    pub sender: bool,
    /// UTXO realted with Transfer
    #[garde(length(min = 0, max = 999))]
    pub utxos: Vec<String>,
    /// List of Beneficiaries(aka. invoices)
    #[garde(skip)]
    pub beneficiaries: Option<BTreeMap<String, String>>,
    /// PSBT related with Transfer
    #[garde(skip)]
    pub psbt: Option<Psbt>,
}

impl RgbInternalSaveTransferRequest {
    pub(crate) fn with(
        consig_id: String,
        consig: String,
        beneficiary: String,
        iface: String,
        sender: bool,
        beneficiaries: Option<BTreeMap<String, String>>,
        psbt: Option<Psbt>,
    ) -> Self {
        let mut utxos = vec![];
        if let Some(psbt) = psbt.clone() {
            utxos = psbt
                .inputs
                .into_iter()
                .map(|x| x.previous_outpoint.to_string())
                .collect();
        }

        Self {
            consig_id,
            iface,
            consig,
            beneficiary,
            sender,
            utxos,
            beneficiaries,
            psbt,
        }
    }
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
pub struct RgbReplaceResponse {
    /// Consignment ID
    pub consig_id: String,
    /// Consignment encoded (in hexadecimal)
    pub consig: String,
    /// PSBT File Information with tapret (in hexadecimal)
    pub psbt: String,
    /// Tapret Commitment (used to spend output)
    pub commit: String,
    /// Strict Consignments (in hexadecimal)
    pub consigs: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbInternalTransferResponse {
    /// Consignment ID
    pub consig_id: String,
    /// Consignment encoded (in hexadecimal)
    pub consig: String,
    /// PSBT File Information with tapret (in hexadecimal)
    pub psbt: String,
    /// Outpoint (used to spend output)
    pub outpoint: String,
    /// Outpoint Amount (used to spend output)
    pub amount: u64,
    /// Tapret Commitment (used to spend output)
    pub commit: String,
    /// Strict Consignments (in hexadecimal)
    pub consigs: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBContext))]
pub struct AcceptRequest {
    /// Consignment encoded in hexadecimal
    #[garde(ascii)]
    #[garde(length(min = 0, max = usize::MAX))]
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
    #[garde(length(min = 0, max = usize::MAX))]
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

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Default)]
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

impl Default for AllocationValue {
    fn default() -> Self {
        AllocationValue::Value(0)
    }
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
    pub is_mine: bool,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Display)]
#[display("{utxo}:{is_spent}")]
pub struct UtxoSpentStatus {
    pub utxo: String,
    pub is_spent: bool,
    pub block_height: TxStatus,
    pub spent_height: TxStatus,
}

impl UtxoSpentStatus {
    pub fn is_invalid_state(self) -> bool {
        matches!(
            (self.block_height, self.spent_height),
            (TxStatus::Error(_), _) | (_, TxStatus::Error(_))
        )
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBContext))]
#[serde(rename_all = "camelCase")]
#[display("{contract_id}:{contract_amount} ** {change_terminal}")]
pub struct RgbOfferRequest {
    /// The Contract ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// The Contract Interface
    #[garde(ascii)]
    #[garde(length(min = 0, max = 32))]
    pub iface: String,
    /// Contract Amount
    #[garde(skip)]
    pub contract_amount: String,
    /// Bitcoin Price (in sats)
    #[garde(range(min = u64::MIN, max = u64::MAX))]
    pub bitcoin_price: u64,
    /// Universal Descriptor
    #[garde(custom(verify_descriptor))]
    pub descriptor: SecretString,
    /// Asset Terminal Change
    #[garde(ascii)]
    pub change_terminal: String,
    /// Bitcoin Change Addresses (format: {address}:{amount})
    #[garde(length(min = 0, max = 999))]
    pub bitcoin_changes: Vec<String>,
    #[garde(skip)]
    pub presig: bool,
    #[garde(skip)]
    pub expire_at: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{offer_id}:{contract_amount} = {bitcoin_price}")]
pub struct RgbOfferResponse {
    /// The Contract ID
    pub offer_id: String,
    /// The Contract ID
    pub contract_id: String,
    /// Contract Amount
    pub contract_amount: f64,
    /// Bitcoin Price
    pub bitcoin_price: u64,
    /// Seller Address
    pub seller_address: String,
    /// Seller PSBT (encoded in base64)
    pub seller_psbt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBContext))]
#[serde(rename_all = "camelCase")]
#[display("{contract_id}:{offer_id}")]
pub struct RgbOfferUpdateRequest {
    /// The Contract ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// The Offer ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: String,
    /// Swap PSBT
    #[garde(ascii)]
    pub offer_psbt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{offer_id}:{updated}")]
pub struct RgbOfferUpdateResponse {
    /// The Contract ID
    pub contract_id: String,
    /// The Offer ID
    pub offer_id: String,
    /// Updated?
    pub updated: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBContext))]
#[serde(rename_all = "camelCase")]
#[display("{offer_id}:{asset_amount} ** {change_terminal}")]
pub struct RgbBidRequest {
    /// The Offer ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: String,
    /// Asset Amount
    #[garde(skip)]
    pub asset_amount: String,
    /// Universal Descriptor
    #[garde(custom(verify_descriptor))]
    pub descriptor: SecretString,
    /// Bitcoin Terminal Change
    #[garde(ascii)]
    pub change_terminal: String,
    /// Bitcoin Fee
    #[garde(dive)]
    pub fee: PsbtFeeRequest,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{bid_id} ~ {offer_id}")]
pub struct RgbBidResponse {
    /// The Bid ID
    pub bid_id: String,
    /// The Offer ID
    pub offer_id: String,
    /// Buyer Invoice
    pub invoice: String,
    /// Final PSBT (encoded in base64)
    pub swap_psbt: String,
    /// Fee Value
    pub fee_value: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBContext))]
#[serde(rename_all = "camelCase")]
#[display("{offer_id} ~ {bid_id}")]
pub struct RgbSwapRequest {
    /// Offer ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: String,
    /// Bid ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bid_id: String,
    /// Swap PSBT
    #[garde(ascii)]
    pub swap_psbt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{consig_id}")]
pub struct RgbSwapResponse {
    /// Transfer ID
    pub consig_id: String,
    /// Final Consig
    pub final_consig: String,
    /// Final PSBT
    pub final_psbt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{offers:?}")]
pub struct PublicRgbOffersResponse {
    /// Public Offers
    pub offers: Vec<PublicRgbOfferResponse>,

    /// Public Bids
    pub bids: BTreeMap<String, Vec<PublicRgbBidResponse>>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{offer_id} ~ {contract_id}:{asset_amount} = {bitcoin_price}")]
pub struct PublicRgbOfferResponse {
    /// Offer ID
    offer_id: String,
    /// Contract ID
    contract_id: String,
    /// Offer PubKey
    offer_pub: String,
    /// Asset/Contract Amount
    asset_amount: u64,
    /// Bitcoin Price
    bitcoin_price: u64,
    /// Initial Offer PSBT
    offer_psbt: String,
}

impl From<RgbOfferSwap> for PublicRgbOfferResponse {
    fn from(value: RgbOfferSwap) -> Self {
        Self {
            contract_id: value.contract_id,
            offer_id: value.offer_id,
            asset_amount: value.asset_amount,
            bitcoin_price: value.bitcoin_price,
            offer_pub: value.public,
            offer_psbt: value.seller_psbt,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{bid_id}:{asset_amount} = {bitcoin_price}")]
pub struct PublicRgbBidResponse {
    /// Bid ID
    bid_id: String,
    /// Asset/Contract Amount
    asset_amount: u64,
    /// Bitcoin Price
    bitcoin_price: u64,
}

impl From<PublicRgbBid> for PublicRgbBidResponse {
    fn from(value: PublicRgbBid) -> Self {
        let PublicRgbBid {
            bid_id,
            asset_amount,
            bitcoin_amount,
            ..
        } = value;

        Self {
            bid_id,
            asset_amount,
            bitcoin_price: bitcoin_amount,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbOfferBidsResponse {
    /// Offers
    pub offers: Vec<RgbOfferDetail>,
    /// bids
    pub bids: Vec<RgbBidDetail>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbOffersResponse {
    /// Offers
    pub offers: Vec<RgbOfferDetail>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbBidsResponse {
    /// Bids
    pub bids: Vec<RgbBidDetail>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{offer_id} ~ {contract_id}:{asset_amount} = {bitcoin_price}")]
pub struct RgbOfferDetail {
    /// Contract ID
    contract_id: String,
    /// Offer ID
    offer_id: String,
    /// Offer Status
    offer_status: String,
    /// Asset/Contract Amount
    asset_amount: u64,
    /// Bitcoin Price
    bitcoin_price: u64,
}

impl From<RgbOffer> for RgbOfferDetail {
    fn from(value: RgbOffer) -> Self {
        Self {
            contract_id: value.contract_id,
            offer_id: value.offer_id,
            offer_status: value.offer_status.to_string(),
            asset_amount: value.asset_amount,
            bitcoin_price: value.bitcoin_price,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{bid_id} ~ {contract_id}:{asset_amount} = {bitcoin_price}")]
pub struct RgbBidDetail {
    /// Contract ID
    contract_id: String,
    /// Bid ID
    bid_id: String,
    /// Offer ID
    offer_id: String,
    /// Bid Status
    bid_status: String,
    /// Asset/Contract Amount
    asset_amount: u64,
    /// Bitcoin Price (in satoshis)
    bitcoin_price: u64,
}

impl From<RgbBid> for RgbBidDetail {
    fn from(value: RgbBid) -> Self {
        Self {
            contract_id: value.contract_id,
            offer_id: value.offer_id,
            bid_id: value.bid_id,
            bid_status: value.bid_status.to_string(),
            asset_amount: value.asset_amount,
            bitcoin_price: value.bitcoin_amount,
        }
    }
}
