// Methods meant to work with RGB contracts defined within the web::rgb module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import * as BMC from "./bitmask_core";

export const issueContract = async (
  nostrHexSk: string,
  request: IssueRequest
): Promise<IssueResponse> =>
  JSON.parse(await BMC.issue_contract(nostrHexSk, request));

export const createInvoice = async (
  nostrHexSk: string,
  request: InvoiceRequest
): Promise<InvoiceResponse> =>
  JSON.parse(await BMC.rgb_create_invoice(nostrHexSk, request));

export const createPsbt = async (
  nostrHexSk: string,
  request: PsbtRequest
): Promise<PsbtResponse> =>
  JSON.parse(await BMC.create_psbt(nostrHexSk, request));

export const psbtSignFile = async (
  nostrHexSk: string,
  request: SignPsbtRequest
): Promise<SignPsbtResponse> =>
  JSON.parse(await BMC.psbt_sign_file(nostrHexSk, request));

export const transferAsset = async (
  nostrHexSk: string,
  request: RgbTransferRequest
): Promise<RgbTransferResponse> =>
  JSON.parse(await BMC.transfer_asset(nostrHexSk, request));

export const fullTransferAsset = async (
  nostrHexSk: string,
  request: FullRgbTransferRequest
): Promise<RgbTransferResponse> =>
  JSON.parse(await BMC.full_transfer_asset(nostrHexSk, request));

export const importContract = async (
  nostrHexSk: string,
  request: ImportRequest
): Promise<ImportResponse> =>
  JSON.parse(await BMC.import_contract(nostrHexSk, request));

export const acceptTransfer = async (
  nostrHexSk: string,
  request: AcceptRequest
): Promise<AcceptResponse> =>
  JSON.parse(await BMC.accept_transfer(nostrHexSk, request));

export const listContracts = async (
  nostrHexSk: string
): Promise<ContractsResponse> =>
  JSON.parse(await BMC.list_contracts(nostrHexSk));

export const listInterfaces = async (
  nostrHexSk: string
): Promise<InterfacesResponse> =>
  JSON.parse(await BMC.list_interfaces(nostrHexSk));

export const listSchemas = async (
  nostrHexSk: string
): Promise<SchemasResponse> => JSON.parse(await BMC.list_schemas(nostrHexSk));

export const createWatcher = async (
  nostrHexSk: string,
  request: WatcherRequest
): Promise<WatcherResponse> =>
  JSON.parse(await BMC.create_watcher(nostrHexSk, request));

export const watcherDetails = async (
  nostrHexSk: string,
  name: string
): Promise<WatcherDetailResponse> =>
  JSON.parse(await BMC.watcher_details(nostrHexSk, name));

export const watcherNextAddress = async (
  nostrHexSk: string,
  name: string,
  iface: string
): Promise<NextAddressResponse> =>
  JSON.parse(await BMC.watcher_next_address(nostrHexSk, name, iface));

export const watcherNextUtxo = async (
  nostrHexSk: string,
  name: string,
  iface: string
): Promise<NextUtxoResponse> =>
  JSON.parse(await BMC.watcher_next_utxo(nostrHexSk, name, iface));

export const watcherUnspentUtxos = async (
  nostrHexSk: string,
  name: string,
  iface: string
): Promise<NextUtxosResponse> =>
  JSON.parse(await BMC.watcher_unspent_utxos(nostrHexSk, name, iface));

export const watcherAddress = async (
  nostrHexSk: string,
  name: string,
  address: string
): Promise<WatcherUtxoResponse> =>
  JSON.parse(await BMC.watcher_address(nostrHexSk, name, address));

export const watcherUtxo = async (
  nostrHexSk: string,
  name: string,
  utxo: string
): Promise<WatcherUtxoResponse> =>
  JSON.parse(await BMC.watcher_utxo(nostrHexSk, name, utxo));

export const listTransfers = async (
  nostrHexSk: string,
  contractId: string
): Promise<RgbTransfersResponse> =>
  JSON.parse(await BMC.list_transfers(nostrHexSk, contractId));

export const saveTransfer = async (
  nostrHexSk: string,
  request: RgbSaveTransferRequest
): Promise<RgbTransferStatusResponse> =>
  JSON.parse(await BMC.save_transfer(nostrHexSk, request));

export const removeTransfer = async (
  nostrHexSk: string,
  request: RgbRemoveTransferRequest
): Promise<RgbTransferStatusResponse> =>
  JSON.parse(await BMC.remove_transfer(nostrHexSk, request));

export const verifyTransfers = async (
  nostrHexSk: string
): Promise<BatchRgbTransferResponse> =>
  JSON.parse(await BMC.verify_transfers(nostrHexSk));

export const decodeInvoice = async (
  invoice: string
): Promise<RgbInvoiceResponse> => JSON.parse(await BMC.decode_invoice(invoice));

// Core type interfaces based on structs defined within the bitmask-core Rust crate:
// https://github.com/diba-io/bitmask-core/blob/development/src/structs.rs

export interface ContractFormats {
  /// The contract state (encoded in bech32m)
  legacy: string;
  /// The contract state (encoded in strict)
  strict: string;
  /// The contract state (compiled in armored mode)
  armored: string;
}

export interface GenesisFormats {
  /// The genesis state (encoded in bech32m)
  legacy: string;
  /// The genesis state (encoded in strict)
  strict: string;
  /// The contract state (compiled in armored mode)
  armored: string;
}

export interface IssueMetadata {
  uda?: MediaInfo[];
  collectible?: NewCollectible[];
}

export interface IssueRequest {
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// Amount of the asset
  supply: bigint;
  /// Precision of the asset
  precision: number;
  /// Seal of the initial owner
  seal: string;
  /// The name of the iface (ex: RGB20)
  iface: string;
  /// contract metadata (only RGB21/UDA)
  meta?: IssueMetadata;
}

export interface NewCollectible {
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// attachments and media
  media: MediaInfo[];
}

export interface UDADetail {
  /// the token index of the uda
  tokenIndex: number;
  /// The ticker of the uda
  ticker: string;
  /// Name of the uda
  name: string;
  /// Description of the uda
  description: string;
  /// Media of the uda
  media: MediaInfo[];
  /// The user contract balance
  balance: bigint;
  /// The contract allocations
  allocations: AllocationDetail[];
}

export interface ContractMetadata {
  uda?: UDADetail;
  collectible?: UDADetail[];
}

export interface IssueResponse {
  /// The contract id
  contractId: string;
  /// The contract impl id
  iimplId: string;
  /// The contract interface
  iface: string;
  /// The Issue Utxo
  issueUtxo: string;
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// Amount of the asset
  supply: bigint;
  /// Precision of the asset
  precision: number;
  /// The contract state (multiple formats)
  contract: ContractFormats;
  /// Genesis
  genesis: GenesisFormats;
  /// attachments and media (only RGB21/UDA)
  meta?: ContractMetadata;
}

export interface ImportRequest {
  /// The type data
  /// enum ImportType {
  ///     "contract"
  /// }
  import: string;
  /// The payload data (in hexadecimal)
  data: string;
}

// In structs.rs this is called ContractResponse
export interface ImportResponse {
  /// The contract id
  contractId: string;
  /// The contract impl id
  iimplId: string;
  /// The contract interface
  iface: string;
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// Amount of the asset
  supply: bigint;
  /// Precision of the asset
  precision: number;
  /// The user contract balance
  balance: bigint;
  /// The contract allocations
  allocations: AllocationDetail[];
  /// The contract state (multiple formats)
  contract: ContractFormats;
  /// Genesis
  genesis: GenesisFormats;
  /// attachments and media (only RGB21/UDA)
  meta?: ContractMetadata;
}

export interface MediaInfo {
  /// Mime Type of the media
  type: string;
  /// Source (aka. hyperlink) of the media
  source: string;
}

export interface InvoiceRequest {
  /// The contract id
  contractId: string;
  /// The contract interface
  iface: string;
  /// Amount of the asset
  amount: bigint;
  /// UTXO or Blinded UTXO
  seal: string;
  /// Query parameters
  params: { [key: string]: string };
}

export interface InvoiceResponse {
  /// Invoice encoded in Baid58
  invoice: string;
}

export interface PsbtRequest {
  /// Asset UTXOs
  asset_inputs: PsbtInputRequest[];
  /// Asset Descriptor Change
  asset_descriptor_change: string;
  /// Asset Terminal Change (default: /10/0)
  asset_terminal_change: string;
  /// Bitcoin UTXOs
  bitcoin_inputs: PsbtInputRequest[];
  /// Bitcoin Change Addresses (format: {address}:{amount})
  bitcoin_changes: string[];
  /// Bitcoin Fee
  fee: PsbtFeeRequest;
}

interface PsbtInputRequest {
  /// Asset or Bitcoin Descriptor
  descriptor: string;
  /// Asset or Bitcoin UTXO
  utxo: string;
  /// Asset or Bitcoin UTXO Terminal (ex. /0/0)
  utxo_terminal: string;
  /// Asset or Bitcoin Tweak
  tapret?: string;
}

interface PsbtFeeRequest {
  value?: number;
  feeRate?: number;
}

export interface PsbtResponse {
  /// PSBT encoded in Base64
  psbt: string;
  /// Asset UTXO Terminal (ex. /0/0)
  terminal: string;
}

export interface SignPsbtRequest {
  /// PSBT encoded in Base64
  psbt: string;
  /// Descriptors to Sign
  descriptors: string[];
}

export interface SignPsbtResponse {
  /// PSBT is signed?
  sign: boolean;
  /// TX id
  txid: string;
}

export interface RgbTransferRequest {
  /// RGB Invoice
  rgbInvoice: string;
  /// PSBT File Information
  psbt: string;
  /// Asset UTXO Terminal (ex. /0/0)
  terminal: string;
}

export interface FullRgbTransferRequest {
  /// The contract id
  contractId: string;
  /// The contract interface
  iface: string;
  /// RGB Invoice
  rgbInvoice: string;
  /// Asset or Bitcoin Descriptor
  descriptor: string;
  /// Bitcoin Terminal Change
  changeTerminal: string;
  /// Bitcoin Fee
  fee: PsbtFeeRequest;
  /// Bitcoin Change Addresses (format: {address}:{amount})
  bitcoinChanges: string[];
}

export interface RgbTransferResponse {
  /// Consignment ID
  consigId: string;
  /// Consignment encoded (in hexadecimal)
  consig: string;
  /// PSBT File Information with tapret (in hexadecimal)
  psbt: string;
  /// Tapret Commitment (used to spend output)
  commit: string;
}

export interface AcceptRequest {
  /// Consignment encoded in hexadecimal
  consignment: string;
  /// Force Consignment accept
  force: boolean;
}

export interface AcceptResponse {
  /// Transfer ID
  transferId: string;
  /// Contract ID
  contractId: string;
  /// Transfer accept status
  valid: boolean;
}

export interface ContractsResponse {
  /// List of avaliable contracts
  contracts: ImportResponse[];
}

export interface InterfacesResponse {
  /// List of avaliable interfaces and implementations
  interfaces: InterfaceDetail[];
}

export interface InterfaceDetail {
  /// Interface Name
  name: string;
  /// Interface ID
  iface: string;
  /// Interface ID
  iimpl: string;
}

export interface SchemasResponse {
  /// List of avaliable schemas
  schemas: SchemaDetail[];
}

export interface SchemaDetail {
  /// Schema ID
  schema: string;
  /// Avaliable Interfaces
  ifaces: string[];
}

export interface WatcherRequest {
  /// The watcher name
  name: string;
  /// The xpub will be watch
  xpub: string;
  /// Force recreate
  force: boolean;
}

export interface WatcherResponse {
  /// The watcher name
  name: string;
}

export interface UtxoResponse {
  outpoint: string;
  amount: bigint;
  status: TxStatus;
}

export interface NextUtxoResponse {
  utxo?: UtxoResponse;
}

export interface NextUtxosResponse {
  utxos: UtxoResponse[];
}

export interface NextAddressResponse {
  address: string;
  network: string;
}

export interface WatcherDetailResponse {
  /// Allocations
  contracts: WatcherDetail[];
}

export interface WatcherUtxoResponse {
  utxos: string[];
}

export interface WatcherDetail {
  /// Contract ID
  contractId: string;
  /// Allocations
  allocations: AllocationDetail[];
}

export interface UDAPosition {
  tokenIndex: number;
  fraction: bigint;
}

export type AllocationValue = {
  value?: bigint;
  uda?: UDAPosition;
};

export interface AllocationDetail {
  /// Anchored UTXO
  utxo: string;
  /// Asset Value
  value: AllocationValue;
  /// Derivation Path
  derivation: string;
  /// Derivation Path
  isMine: boolean;
  /// Allocation spent?
  isSpent: boolean;
}

export interface DeclareRequest {
  disclosure: string;
  // disclosure: Declare; // TODO: Revisit after 0.6 release
  changeTransfers: ChangeTansfer[];
  transfers: FinalizeTransfer[];
}

export interface FinalizeTransfer {
  previousUtxo: string;
  consignment: string;
  asset: string;
  beneficiaries: BlindedOrNotOutpoint[];
}

export interface ChangeTansfer {
  previousUtxo: string;
  asset: string;
  change: BlindedOrNotOutpoint;
}

export interface BlindedOrNotOutpoint {
  outpoint: string;
  balance: number;
}

export interface Contract {
  id: string;
  ticker: string;
  name: string;
  description: string;
  allocations: AllocationDetail[];
  balance: bigint;
  genesis: string;
}

export interface RgbSaveTransferRequest {
  /// iFace Name
  iface: string;
  /// Consignment encoded in hexadecimal
  consignment: string;
}

export interface RgbRemoveTransferRequest {
  /// Contract ID
  contractId: string;
  /// Consignment ID
  consigIds: string[];
}

export interface RgbTransferStatusResponse {
  /// Contract ID
  contractId: string;
  /// Transfer ID
  consigStatus: Map<string, boolean>;
}

export interface RgbTransfersResponse {
  /// List of avaliable transfers
  transfers: RgbTransferDetail[];
}

export interface RgbTransferDetail {
  consigId: string;
  status: TxStatus;
  type: TransferType;
}

export interface TxStatus {
  not_found?: any;
  error?: string;
  mempool?: any;
  block?: number;
}

export interface TransferType {
  sended?: any;
  received?: any;
  unknown?: any;
}

export interface RgbInvoiceResponse {
  contractId: string;
  amount: bigint;
}

export interface BatchRgbTransferResponse {
  transfers: BatchRgbTransferItem[];
}

export interface BatchRgbTransferItem {
  contractId: string;
  consigId: string;
  status: TxStatus;
  isAccept: boolean;
  iface: string;
}