use ::psbt::{serialize::Serialize, Psbt};
use amplify::{confinement::U32, hex::ToHex};
use anyhow::Result;
use autosurgeon::reconcile;
use bitcoin::{psbt::PartiallySignedTransaction as PsbtV0, Network, Txid};
use bitcoin_30::bip32::ExtendedPubKey;
use bitcoin_scripts::address::AddressNetwork;
use futures::TryFutureExt;
use garde::Validate;

use miniscript_crate::DescriptorPublicKey;
use rgb::RgbDescr;
use rgbstd::{
    containers::BindleContent,
    contract::ContractId,
    interface::TypedState,
    persistence::{Inventory, Stash, Stock},
    validation::Validity,
};
use rgbwallet::{psbt::DbcPsbtError, RgbInvoice};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    ops::Sub,
    str::FromStr,
};
use strict_encoding::{tn, StrictSerialize};
use thiserror::Error;

pub mod accept;
pub mod cambria;
pub mod carbonado;
pub mod consignment;
pub mod constants;
pub mod contract;
pub mod crdt;
pub mod fs;
pub mod import;
pub mod issue;
pub mod prebuild;
pub mod prefetch;
pub mod proxy;
pub mod psbt;
pub mod resolvers;
pub mod structs;
pub mod swap;
pub mod transfer;
pub mod wallet;

use crate::{
    bitcoin::{publish_psbt_file, sign_psbt_file},
    constants::{get_network, BITCOIN_EXPLORER_API, NETWORK},
    rgb::{
        issue::{issue_contract as create_contract, IssueContractError},
        psbt::{create_psbt as create_rgb_psbt, extract_output_commit},
        resolvers::ExplorerResolver,
        transfer::{
            accept_transfer as accept_rgb_transfer, create_invoice as create_rgb_invoice,
            pay_invoice,
        },
        wallet::list_allocations,
    },
    structs::{
        AcceptRequest, AcceptResponse, AssetType, BatchRgbTransferItem, BatchRgbTransferResponse,
        ContractHiddenResponse, ContractResponse, ContractsResponse, FullRgbTransferRequest,
        ImportRequest, InterfaceDetail, InterfacesResponse, InvoiceRequest, InvoiceResponse,
        IssueMediaRequest, IssueRequest, IssueResponse, MediaEncode, MediaRequest, MediaResponse,
        MediaView, NextAddressResponse, NextUtxoResponse, NextUtxosResponse, PsbtFeeRequest,
        PsbtRequest, PsbtResponse, PublicRgbBidResponse, PublicRgbOfferResponse,
        PublicRgbOffersResponse, PublishPsbtRequest, ReIssueRequest, ReIssueResponse,
        RgbAuctionBidRequest, RgbAuctionBidResponse, RgbAuctionFinishResponse,
        RgbAuctionOfferRequest, RgbAuctionOfferResponse, RgbBidDetail, RgbBidRequest,
        RgbBidResponse, RgbBidsResponse, RgbInternalSaveTransferRequest,
        RgbInternalTransferResponse, RgbInvoiceResponse, RgbOfferBidsResponse, RgbOfferDetail,
        RgbOfferRequest, RgbOfferResponse, RgbOfferUpdateRequest, RgbOfferUpdateResponse,
        RgbOffersResponse, RgbRemoveTransferRequest, RgbReplaceResponse, RgbSaveTransferRequest,
        RgbSwapItem, RgbSwapRequest, RgbSwapResponse, RgbTransferDetail, RgbTransferRequest,
        RgbTransferResponse, RgbTransferStatusResponse, RgbTransfersResponse, SchemaDetail,
        SchemasResponse, SignPsbtRequest, SignedPsbtResponse, SimpleContractResponse, TransferType,
        TxStatus, UtxoResponse, WatcherDetailResponse, WatcherRequest, WatcherResponse,
        WatcherUtxoResponse,
    },
    validators::RGBContext,
};

use self::{
    consignment::NewTransferOptions,
    constants::{RGB_DEFAULT_FETCH_LIMIT, RGB_DEFAULT_NAME},
    contract::{export_boilerplate, export_contract, extract_metadata, ExportContractError},
    crdt::{LocalRgbAccount, RawRgbAccount, RgbMerge},
    fs::{
        retrieve_account, retrieve_bids, retrieve_local_account, retrieve_offers,
        retrieve_public_offers, retrieve_stock as retrieve_rgb_stock, retrieve_stock_account,
        retrieve_stock_account_transfers, retrieve_stock_transfers, retrieve_transfers,
        store_account, store_bids, store_local_account, store_offers,
        store_stock as store_rgb_stock, store_stock_account, store_stock_account_transfers,
        store_stock_transfers, store_transfers, RgbPersistenceError,
    },
    import::{import_contract, ImportContractError},
    prebuild::{
        prebuild_buyer_swap, prebuild_extract_transfer, prebuild_seller_swap,
        prebuild_transfer_asset,
    },
    prefetch::{
        prefetch_resolver_allocations, prefetch_resolver_import_rgb, prefetch_resolver_psbt,
        prefetch_resolver_rgb, prefetch_resolver_txs_status, prefetch_resolver_user_utxo_status,
        prefetch_resolver_utxos, prefetch_resolver_waddress, prefetch_resolver_wutxo,
    },
    proxy::{
        get_consignment as get_rgb_consignment, get_media_metadata as get_rgb_media_metadata,
        post_consignments, post_media_metadata, post_media_metadata_list, ProxyError,
    },
    psbt::{
        save_rgb_commit_str, set_tapret_output, CreatePsbtError, EstimateFeeError, NewPsbtOptions,
    },
    structs::{
        ContractAmount, ContractBoilerplate, MediaMetadata, RgbAccountV1, RgbExtractTransfer,
        RgbTransferV1, RgbTransfersV1,
    },
    swap::{
        complete_bid, complete_offer, get_auction, get_auction_fifo_bids, get_auction_highest_bids,
        get_auction_offer, get_public_offer, get_public_offers, get_swap_bid_by_buyer,
        get_swap_bid_by_seller, get_swap_bids_by_offer, publish_auction_bid,
        publish_auction_offers, publish_public_bid, publish_public_offer, publish_swap_bid,
        remove_public_offers, update_transfer_bid, update_transfer_offer, PsbtSwapEx,
        RgbAuctionStrategy, RgbBid, RgbBidSwap, RgbOffer, RgbOfferErrors, RgbOfferOptions,
        RgbOfferSwap, RgbSwapStrategy,
    },
    transfer::{extract_transfer, AcceptTransferError, NewInvoiceError, NewPaymentError},
    wallet::{
        create_wallet, next_address, next_utxo, next_utxos, register_address, register_utxo,
        sync_wallet,
    },
};

#[derive(Debug, Clone, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum IssueError {
    /// Some request data is missing. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(RgbPersistenceError),
    /// Proxy connectivity error. {0}
    Proxy(ProxyError),
    /// Watcher is required for this operation.
    Watcher,
    /// Occurs an error in issue step. {0}
    Issue(IssueContractError),
    /// Occurs an error in export step. {0}
    Export(ExportContractError),
}

/// RGB Operations
pub async fn issue_contract(sk: &str, request: IssueRequest) -> Result<IssueResponse, IssueError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(IssueError::Validation(errors));
    }

    let IssueRequest {
        ticker,
        name,
        description,
        supply,
        precision,
        iface,
        seal,
        meta,
    } = request;

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) = retrieve_stock_account(sk).await.map_err(IssueError::IO)?;
    let network = get_network().await;
    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                let contract_index = contract_type as u32;
                prefetch_resolver_utxos(
                    contract_index,
                    &mut fetch_wallet,
                    &mut resolver,
                    Some(RGB_DEFAULT_FETCH_LIMIT),
                )
                .await;
                prefetch_resolver_user_utxo_status(
                    contract_index,
                    &mut fetch_wallet,
                    &mut resolver,
                    true,
                )
                .await;
            }

            Some(fetch_wallet)
        }
        _ => None,
    };

    let contract_amount = ContractAmount::new(supply, precision);
    let contract = create_contract(
        &ticker,
        &name,
        &description,
        precision,
        contract_amount.to_value(),
        &iface,
        &seal,
        &network,
        meta,
        &mut resolver,
        &mut stock,
    )
    .map_err(IssueError::Issue)?;

    let ContractResponse {
        contract_id,
        iimpl_id,
        iface,
        ticker,
        name,
        description,
        supply,
        contract,
        genesis,
        meta,
        created,
        ..
    } = export_contract(
        contract.contract_id(),
        &mut stock,
        &mut resolver,
        &mut wallet,
    )
    .map_err(IssueError::Export)?;

    let meta = if let Some(metadata) = meta {
        Some(
            extract_metadata(metadata)
                .map_err(IssueError::Proxy)
                .await?,
        )
    } else {
        None
    };

    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);
    };

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(IssueError::IO)?;

    Ok(IssueResponse {
        contract_id,
        iface,
        iimpl_id,
        ticker,
        name,
        description,
        supply,
        precision,
        contract,
        genesis,
        created,
        issue_method: "tapret1st".to_string(),
        issue_utxo: seal.replace("tapret1st:", ""),
        meta,
    })
}

pub async fn reissue_contract(
    sk: &str,
    request: ReIssueRequest,
) -> Result<ReIssueResponse, IssueError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(IssueError::Validation(errors));
    }

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) = retrieve_stock_account(sk).await.map_err(IssueError::IO)?;

    let mut reissue_resp = vec![];
    for contract in request.contracts {
        let ContractResponse {
            ticker,
            name,
            description,
            supply,
            iface,
            precision,
            allocations,
            meta: contract_meta,
            ..
        } = contract;

        let seals: Vec<String> = allocations
            .into_iter()
            .map(|alloc| format!("tapret1st:{}", alloc.utxo))
            .collect();
        let seal = seals.first().unwrap().to_owned();

        // TODO: Move to rgb/issue sub-module
        let meta = contract_meta.map(IssueMediaRequest::from);
        let network = get_network().await;
        let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
        let mut wallet = match wallet {
            Some(wallet) => {
                let mut fetch_wallet = wallet.to_owned();
                for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                    prefetch_resolver_utxos(
                        contract_type as u32,
                        &mut fetch_wallet,
                        &mut resolver,
                        Some(RGB_DEFAULT_FETCH_LIMIT),
                    )
                    .await;
                }

                Some(fetch_wallet)
            }
            _ => None,
        };

        let contract = create_contract(
            &ticker,
            &name,
            &description,
            precision,
            supply,
            &iface,
            &seal,
            &network,
            meta,
            &mut resolver,
            &mut stock,
        )
        .map_err(IssueError::Issue)?;

        let ContractResponse {
            contract_id,
            iimpl_id,
            iface,
            ticker,
            name,
            description,
            supply,
            contract,
            genesis,
            meta,
            created,
            ..
        } = export_contract(
            contract.contract_id(),
            &mut stock,
            &mut resolver,
            &mut wallet,
        )
        .map_err(IssueError::Export)?;

        if let Some(wallet) = wallet {
            rgb_account
                .wallets
                .insert(RGB_DEFAULT_NAME.to_string(), wallet);
        };

        let meta = if let Some(metadata) = meta {
            Some(
                extract_metadata(metadata)
                    .map_err(IssueError::Proxy)
                    .await?,
            )
        } else {
            None
        };

        reissue_resp.push(IssueResponse {
            contract_id,
            iface,
            iimpl_id,
            ticker,
            name,
            description,
            supply,
            precision,
            contract,
            genesis,
            created,
            issue_method: "tapret1st".to_string(),
            issue_utxo: seal.replace("tapret1st:", ""),
            meta,
        });
    }

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(IssueError::IO)?;

    Ok(ReIssueResponse {
        contracts: reissue_resp,
    })
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum InvoiceError {
    /// Some request data is missing. {0:?}
    Validation(BTreeMap<String, String>),
    /// Contract is required in this operation. Please, import or issue a Contract.
    NoContract,
    /// Invoice contains wrong contract precision. expect: {0} / current: {1}.
    WrongPrecision(u8, u8),
    /// I/O or connectivity error. {0}
    IO(RgbPersistenceError),
    /// Occurs an error in invoice step. {0}
    Invoice(NewInvoiceError),
}

pub async fn create_invoice(
    sk: &str,
    request: InvoiceRequest,
) -> Result<InvoiceResponse, InvoiceError> {
    let (mut stock, mut rgb_account) =
        retrieve_stock_account(sk).await.map_err(InvoiceError::IO)?;

    let invoice = internal_create_invoice(request, &mut stock).await?;
    rgb_account.invoices.push(invoice.to_string());

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(InvoiceError::IO)?;

    Ok(InvoiceResponse {
        invoice: invoice.to_string(),
    })
}

async fn internal_create_invoice(
    request: InvoiceRequest,
    stock: &mut Stock,
) -> Result<RgbInvoice, InvoiceError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(InvoiceError::Validation(errors));
    }

    let InvoiceRequest {
        contract_id,
        iface,
        seal,
        amount,
        params,
    } = request;

    let network = NETWORK.read().await.to_string();

    let contr_id = ContractId::from_str(&contract_id).map_err(|_| InvoiceError::NoContract)?;
    let boilerplate = export_boilerplate(contr_id, stock).map_err(|_| InvoiceError::NoContract)?;
    let invoice_amount = ContractAmount::from_decimal_str(amount.to_string());
    if invoice_amount.precision != boilerplate.precision {
        return Err(InvoiceError::WrongPrecision(
            boilerplate.precision,
            invoice_amount.precision,
        ));
    }

    let invoice_amount = invoice_amount.to_value();
    let invoice = create_rgb_invoice(
        &contract_id,
        &iface,
        invoice_amount,
        &seal,
        &network,
        params,
        stock,
    )
    .map_err(InvoiceError::Invoice)?;

    Ok(invoice)
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum PsbtError {
    /// Some request data is missing. {0:?}
    Validation(BTreeMap<String, String>),
    /// Retrieve I/O or connectivity error. {0:?}
    IO(RgbPersistenceError),
    /// Watcher is required in this operation. Please, create watcher.
    NoWatcher,
    /// Contract is required in this operation. Please, import or issue a Contract.
    NoContract,
    /// FeeRate is supported in this operation. Please, use the absolute fee value.
    NoFeeRate,
    /// Insufficient funds (expected: {input} sats / current: {output} sats)
    Inflation {
        /// Amount spent: input amounts
        input: u64,

        /// Amount sent: sum of output value + transaction fee
        output: u64,
    },
    /// Auto merge fail in this opration
    WrongAutoMerge(String),
    /// Occurs an error in create step. {0}
    Create(CreatePsbtError),
    /// Bitcoin network be decoded. {0}
    WrongNetwork(String),
    /// Occurs an error in export step. {0}
    Export(ExportContractError),
}

pub async fn create_psbt(sk: &str, request: PsbtRequest) -> Result<PsbtResponse, PsbtError> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let mut rgb_account = retrieve_account(sk).await.map_err(PsbtError::IO)?;

    let options = NewPsbtOptions::with(request.rbf);
    let psbt =
        internal_create_psbt(request, &mut rgb_account, &mut resolver, Some(options)).await?;
    Ok(psbt)
}

async fn internal_create_psbt(
    request: PsbtRequest,
    rgb_account: &mut RgbAccountV1,
    resolver: &mut ExplorerResolver,
    options: Option<NewPsbtOptions>,
) -> Result<PsbtResponse, PsbtError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(PsbtError::Validation(errors));
    }

    if rgb_account.wallets.get(RGB_DEFAULT_NAME).is_none() {
        return Err(PsbtError::NoWatcher);
    }

    let PsbtRequest {
        asset_inputs,
        asset_terminal_change,
        bitcoin_inputs,
        bitcoin_changes,
        fee,
        ..
    } = request;

    let mut all_inputs = asset_inputs.clone();
    all_inputs.extend(bitcoin_inputs.clone());
    for input_utxo in all_inputs.clone() {
        prefetch_resolver_psbt(&input_utxo.utxo, resolver).await;
    }

    // Retrieve transaction fee
    let fee = match fee {
        PsbtFeeRequest::Value(fee) => fee,
        PsbtFeeRequest::FeeRate(_) => return Err(PsbtError::NoFeeRate),
    };

    let options = options.unwrap_or_default();
    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let (mut psbt_file, change_terminal) = create_rgb_psbt(
        all_inputs,
        bitcoin_changes,
        fee,
        asset_terminal_change,
        wallet.cloned(),
        resolver,
        options.clone(),
    )
    .map_err(PsbtError::Create)?;

    if options.set_tapret {
        let pos = (psbt_file.outputs.len() - 1) as u16;
        psbt_file = set_tapret_output(psbt_file, pos).map_err(PsbtError::Create)?;
    }

    let psbt = PsbtResponse {
        psbt: Serialize::serialize(&psbt_file).to_hex(),
        terminal: change_terminal,
    };

    Ok(psbt)
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum TransferError {
    /// Some request data is missing. {0:?}
    Validation(BTreeMap<String, String>),
    /// Retrieve I/O or connectivity error. {0:?}
    IO(RgbPersistenceError),
    /// Watcher is required in this operation. Please, create watcher.
    NoWatcher,
    /// Contract is required in this operation. Please, import or issue a Contract.
    NoContract,
    /// Iface is required in this operation. Please, use the correct iface contract.
    NoIface,
    /// FeeRate is supported in this operation. Please, use the absolute fee value.
    NoFeeRate,
    /// Insufficient funds (expected: {input} sats / current: {output} sats)
    Inflation {
        /// Amount spent: input amounts
        input: u64,

        /// Amount sent: sum of output value + transaction fee
        output: u64,
    },
    /// Occurs an error in create step. {0}
    Create(PsbtError),
    /// Occurs an error in estimate fee step. {0}
    Estimate(EstimateFeeError),
    /// Occurs an error in commitment step. {0}
    Commitment(DbcPsbtError),
    /// Occurs an error in payment step. {0}
    Pay(NewPaymentError),
    /// Occurs an error in accept step. {0}
    Accept(AcceptTransferError),
    /// Auto merge fail in this opration
    WrongAutoMerge(String),
    /// Consignment cannot be encoded.
    WrongConsig(String),
    /// Rgb Invoice cannot be decoded. {0}
    WrongInvoice(String),
    /// Bitcoin network be decoded. {0}
    WrongNetwork(String),
    /// Occurs an error in swap step. {0}
    WrongSwap(RgbOfferErrors),
    /// Occurs an error in save transfer step. {0}
    WrongSave(SaveTransferError),
    /// Occurs an error in export step. {0}
    Export(ExportContractError),
    /// Occurs an error in retrieve transfers step. {0}
    Save(SaveTransferError),
    /// Occurs an error in retrieve proxy step. {0}
    Proxy(ProxyError),
}

pub async fn full_transfer_asset(
    sk: &str,
    request: FullRgbTransferRequest,
) -> Result<RgbTransferResponse, TransferError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(TransferError::Validation(errors));
    }

    let (mut stock, mut rgb_transfers) = retrieve_stock_transfers(sk)
        .await
        .map_err(TransferError::IO)?;

    let local_rgb_account = retrieve_local_account(sk)
        .await
        .map_err(TransferError::IO)?;

    let LocalRgbAccount {
        mut rgb_account,
        version,
    } = local_rgb_account;
    let mut fork_wallet = automerge::AutoCommit::load(&version)
        .map_err(|op| TransferError::WrongAutoMerge(op.to_string()))?;
    let mut rgb_account_changes = RawRgbAccount::from(rgb_account.clone());

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let mut rgb_wallet = match rgb_account.wallets.get(RGB_DEFAULT_NAME) {
        Some(rgb_wallet) => rgb_wallet.to_owned(),
        _ => return Err(TransferError::NoWatcher),
    };

    let (asset_inputs, bitcoin_inputs, bitcoin_changes, fee_value) =
        prebuild_transfer_asset(request.clone(), &mut stock, &mut rgb_wallet, &mut resolver)
            .await?;

    let FullRgbTransferRequest {
        rgb_invoice,
        change_terminal,
        ..
    } = request;

    let psbt_req = PsbtRequest {
        fee: PsbtFeeRequest::Value(fee_value),
        asset_inputs,
        bitcoin_inputs,
        bitcoin_changes,
        asset_descriptor_change: None,
        asset_terminal_change: Some(change_terminal),
        rbf: true,
    };

    let psbt_response = internal_create_psbt(psbt_req, &mut rgb_account, &mut resolver, None)
        .await
        .map_err(TransferError::Create)?;

    let transfer_req = RgbTransferRequest {
        rgb_invoice,
        psbt: psbt_response.psbt,
        terminal: psbt_response.terminal.clone(),
    };

    let options = NewTransferOptions::default();
    let RgbInternalTransferResponse {
        consig_id,
        consig,
        psbt,
        commit,
        outpoint,
        amount,
        txid,
        ..
    } = internal_transfer_asset(
        transfer_req,
        options,
        &mut stock,
        &mut rgb_account,
        &mut rgb_transfers,
    )
    .await?;

    let mut rgb_wallet = match rgb_account.wallets.get(RGB_DEFAULT_NAME) {
        Some(rgb_wallet) => rgb_wallet.to_owned(),
        _ => return Err(TransferError::NoWatcher),
    };

    save_rgb_commit_str(
        &outpoint,
        amount,
        &commit,
        &psbt_response.terminal,
        &mut rgb_wallet,
    );
    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet);

    let resp = RgbTransferResponse {
        consig_id,
        consig,
        psbt,
        commit,
        txid,
    };

    rgb_account.clone().update(&mut rgb_account_changes);
    reconcile(&mut fork_wallet, rgb_account_changes.clone())
        .map_err(|op| TransferError::WrongAutoMerge(op.to_string()))?;

    store_local_account(sk, fork_wallet.save())
        .await
        .map_err(TransferError::IO)?;

    store_stock_transfers(sk, stock, rgb_transfers)
        .await
        .map_err(TransferError::IO)?;

    Ok(resp)
}

pub async fn transfer_asset(
    sk: &str,
    request: RgbTransferRequest,
) -> Result<RgbTransferResponse, TransferError> {
    let (mut stock, mut rgb_account, mut rgb_transfers) = retrieve_stock_account_transfers(sk)
        .await
        .map_err(TransferError::IO)?;

    let options = NewTransferOptions::default();
    let RgbInternalTransferResponse {
        consig_id,
        consig,
        psbt,
        commit,
        outpoint,
        amount,
        txid,
        ..
    } = internal_transfer_asset(
        request.clone(),
        options,
        &mut stock,
        &mut rgb_account,
        &mut rgb_transfers,
    )
    .await?;

    let mut rgb_wallet = match rgb_account.wallets.get(RGB_DEFAULT_NAME) {
        Some(rgb_wallet) => rgb_wallet.to_owned(),
        _ => return Err(TransferError::NoWatcher),
    };

    save_rgb_commit_str(
        &outpoint,
        amount,
        &commit,
        &request.terminal,
        &mut rgb_wallet,
    );
    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet);

    let resp = RgbTransferResponse {
        consig_id,
        consig,
        psbt,
        commit,
        txid,
    };

    store_stock_account_transfers(sk, stock, rgb_account, rgb_transfers)
        .await
        .map_err(TransferError::IO)?;

    Ok(resp)
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbSwapError {
    /// Some request data is missing. {0:?}
    Validation(BTreeMap<String, String>),
    /// Retrieve I/O or connectivity error. {0:?}
    IO(RgbPersistenceError),
    /// Watcher is required in this operation. Please, create watcher.
    NoWatcher,
    /// Contract is required in this operation. Please, import or issue a Contract.
    NoContract,
    /// Available Utxo is required in this operation. {0}
    NoUtxo(String),
    /// Offer {0} not found.
    NoOffer(String),
    /// Bundle {0} not found.
    NoBundle(String),
    /// The Airdrop strategy needs define fixed bitcoin fee value.
    NoAirdropFee,
    /// The Offer has expired.
    OfferExpired,
    /// Insufficient funds (expected: {input} sats / current: {output} sats)
    Inflation {
        /// Amount spent: input amounts
        input: u64,

        /// Amount sent: sum of output value + transaction fee
        output: u64,
    },
    /// Occurs an error in export step. {0}
    Export(ExportContractError),
    /// Occurs an error in create offer buyer step. {0}
    Buyer(RgbOfferErrors),
    /// Occurs an error in create step. {0}
    Create(PsbtError),
    /// Occurs an error in estimate fee step. {0}
    Estimate(EstimateFeeError),
    /// Occurs an error in auction step. {0}
    Auction(RgbOfferErrors),
    /// Occurs an error in public offer step. {0}
    Marketplace(RgbOfferErrors),
    /// Occurs an error in invoice step. {0}
    Invoice(InvoiceError),
    /// Occurs an error in create offer swap step. {0}
    Swap(RgbOfferErrors),
    /// Occurs an error in transfer step. {0}
    Transfer(TransferError),
    /// Offer {0} is valid. Reason {1}
    WrongOffer(String, String),
    /// Swap fee cannot be decoded. {0}
    WrongSwapFee(String),
    /// Request order contains wrong contract precision. expect: {0} / current: {1}.
    WrongPrecision(u8, u8),
    /// Request order contains wrong contract value. {0}.
    WrongValue(String),
    /// Bitcoin network cannot be decoded. {0}
    WrongNetwork(String),
    /// Bitcoin address cannot be decoded. {0}
    WrongAddress(String),
    /// This operation cannot support the {0} strategy.
    WrongStrategy(String),
    /// Seller PSBT cannot be decoded. {0}
    WrongPsbtSeller(String),
    /// Buyer PSBT cannot be decoded. {0}
    WrongPsbtBuyer(String),
    /// PSBTs cannot be merged. {0}
    WrongPsbtSwap(String),
    /// Swap Consig Cannot be decoded. {0}
    WrongConsigSwap(String),
    /// Final PSBT cannot be finished. {0}
    WrongPsbtFinal(String),
}

pub async fn create_seller_offer(
    sk: &str,
    request: RgbOfferRequest,
) -> Result<RgbOfferResponse, RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) =
        retrieve_stock_account(sk).await.map_err(RgbSwapError::IO)?;

    let options = RgbOfferOptions::default();
    let new_offer = internal_create_seller_offer(
        sk,
        request,
        options,
        &mut stock,
        &mut rgb_account,
        &mut resolver,
    )
    .await?;

    let RgbOffer {
        contract_id,
        asset_amount,
        asset_precision,
        seller_address,
        bitcoin_price,
        seller_psbt,
        strategy,
        ..
    } = new_offer.clone();

    let contract_amount = ContractAmount::new(asset_amount, asset_precision).to_string();
    let contract_amount = f64::from_str(&contract_amount).expect("Invalid Contract Amount Value");
    let resp = RgbOfferResponse {
        offer_id: new_offer.clone().offer_id,
        contract_id: contract_id.clone(),
        contract_amount,
        bitcoin_price,
        seller_address,
        seller_psbt,
        bundle_id: None,
    };

    let mut my_offers = retrieve_offers(sk).await.map_err(RgbSwapError::IO)?;
    my_offers = my_offers.save_offer(contract_id, new_offer.clone());

    store_offers(sk, my_offers)
        .await
        .map_err(RgbSwapError::IO)?;

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(RgbSwapError::IO)?;

    let public_offer = RgbOfferSwap::from(new_offer);

    match strategy {
        RgbSwapStrategy::P2P | RgbSwapStrategy::HotSwap => publish_public_offer(public_offer)
            .await
            .map_err(RgbSwapError::Marketplace)?,
        invalid => return Err(RgbSwapError::WrongStrategy(invalid.to_string())),
    }

    Ok(resp)
}

pub async fn create_auction_offers(
    sk: &str,
    request: RgbAuctionOfferRequest,
) -> Result<Vec<RgbOfferResponse>, RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) =
        retrieve_stock_account(sk).await.map_err(RgbSwapError::IO)?;

    let mut my_offers = retrieve_offers(sk).await.map_err(RgbSwapError::IO)?;

    let mut resp = vec![];
    let mut collection = vec![];

    let RgbAuctionOfferRequest {
        offers,
        strategy,
        fee,
        ..
    } = request.clone();

    let options = match strategy {
        RgbAuctionStrategy::Auction => RgbOfferOptions::new(sk.to_owned()),
        RgbAuctionStrategy::Airdrop { max_claim } => {
            if let Some(fee) = fee {
                let contract_amount = ContractAmount::from_decimal_str(max_claim);
                RgbOfferOptions::new_airdrop(sk.to_owned(), fee, contract_amount.to_value())
            } else {
                return Err(RgbSwapError::NoAirdropFee);
            }
        }
    };
    for item in offers {
        let mut new_offer = internal_create_seller_offer(
            sk,
            item,
            options.clone(),
            &mut stock,
            &mut rgb_account,
            &mut resolver,
        )
        .await?;

        let RgbOffer {
            offer_id,
            contract_id,
            asset_amount,
            asset_precision,
            seller_address,
            bitcoin_price,
            seller_psbt,
            strategy,
            bundle_id,
            ..
        } = new_offer.clone();

        if ![RgbSwapStrategy::Auction, RgbSwapStrategy::Airdrop].contains(&strategy) {
            return Err(RgbSwapError::WrongStrategy(strategy.to_string()));
        }

        let contract_amount = ContractAmount::new(asset_amount, asset_precision).to_string();
        let contract_amount =
            f64::from_str(&contract_amount).expect("Invalid Contract Amount Value");

        let request = SignPsbtRequest {
            psbt: seller_psbt,
            descriptors: request.sign_keys.clone(),
        };

        let SignedPsbtResponse {
            psbt: final_psbt, ..
        } = sign_psbt_file(request)
            .await
            .map_err(|op| RgbSwapError::WrongPsbtFinal(op.to_string()))?;
        new_offer.seller_psbt = final_psbt.clone();

        my_offers = my_offers.save_offer(contract_id.clone(), new_offer.clone());
        collection.push(RgbOfferSwap::from(new_offer.clone()));

        resp.push(RgbOfferResponse {
            offer_id,
            contract_id: contract_id.clone(),
            contract_amount,
            bitcoin_price,
            seller_address: seller_address.to_string(),
            seller_psbt: final_psbt,
            bundle_id,
        });
    }

    store_offers(sk, my_offers)
        .await
        .map_err(RgbSwapError::IO)?;

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(RgbSwapError::IO)?;

    publish_auction_offers(request.strategy, collection)
        .await
        .map_err(RgbSwapError::Auction)?;

    Ok(resp)
}

pub async fn update_seller_offer(
    sk: &str,
    request: RgbOfferUpdateRequest,
) -> Result<RgbOfferUpdateResponse, RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let RgbOfferUpdateRequest {
        contract_id,
        offer_id,
        offer_psbt,
        ..
    } = request;

    let mut updated = false;
    let mut my_offers = retrieve_offers(sk).await.map_err(RgbSwapError::IO)?;
    if let Some(offers) = my_offers.offers.get(&contract_id.clone()) {
        let mut current_offers = offers.to_owned();
        if let Some(position) = current_offers.iter().position(|x| x.offer_id == offer_id) {
            let mut offer = current_offers.swap_remove(position);
            offer.seller_psbt = offer_psbt;
            current_offers.insert(position, offer.clone());
            my_offers.offers.insert(contract_id.clone(), current_offers);

            updated = true;
            store_offers(sk, my_offers)
                .await
                .map_err(RgbSwapError::IO)?;

            let public_offer = RgbOfferSwap::from(offer);
            publish_public_offer(public_offer)
                .await
                .map_err(RgbSwapError::Marketplace)?;
        }
    }

    Ok(RgbOfferUpdateResponse {
        contract_id,
        offer_id,
        updated,
    })
}

pub async fn internal_create_seller_offer(
    sk: &str,
    request: RgbOfferRequest,
    options: RgbOfferOptions,
    rgb_stock: &mut Stock,
    rgb_account: &mut RgbAccountV1,
    rgb_resolver: &mut ExplorerResolver,
) -> Result<RgbOffer, RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let network = NETWORK.read().await.to_string();
    let network =
        Network::from_str(&network).map_err(|op| RgbSwapError::WrongNetwork(op.to_string()))?;

    let mut rgb_wallet = match rgb_account.wallets.get(RGB_DEFAULT_NAME) {
        Some(rgb_wallet) => rgb_wallet.to_owned(),
        _ => return Err(RgbSwapError::NoWatcher),
    };

    let RgbOfferRequest {
        contract_id,
        contract_amount,
        bitcoin_price,
        iface,
        expire_at,
        strategy,
        change_terminal,
        ..
    } = request.clone();

    let network = AddressNetwork::from(network);
    let seller_address = next_address(AssetType::Bitcoin as u32, rgb_wallet.clone(), network)
        .map_err(|op| RgbSwapError::WrongAddress(op.to_string()))?;

    let contr_id = ContractId::from_str(&contract_id).unwrap();
    let boilerplate =
        export_boilerplate(contr_id, rgb_stock).map_err(|_| RgbSwapError::NoContract)?;

    let RgbOfferOptions {
        bundle_id,
        max_claim,
        fee_airdrop,
        ..
    } = options.clone();
    let (allocations, asset_inputs, bitcoin_inputs, mut bitcoin_changes, change_value) =
        prebuild_seller_swap(request, options, rgb_stock, &mut rgb_wallet, rgb_resolver).await?;

    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet.clone());

    let (final_fee, psbt_options) = if let Some(fee_airdrop) = fee_airdrop {
        (fee_airdrop, NewPsbtOptions::default())
    } else {
        bitcoin_changes.push(format!("{seller_address}:{bitcoin_price}"));
        (
            PsbtFeeRequest::Value(0),
            NewPsbtOptions::set_inflaction(change_value),
        )
    };

    let psbt_req = PsbtRequest {
        fee: final_fee,
        asset_inputs,
        bitcoin_inputs,
        bitcoin_changes,
        asset_descriptor_change: None,
        asset_terminal_change: Some(change_terminal.clone()),
        rbf: true,
    };

    let seller_psbt = internal_create_psbt(psbt_req, rgb_account, rgb_resolver, Some(psbt_options))
        .await
        .map_err(RgbSwapError::Create)?;

    let contract_amount = ContractAmount::from_decimal_str(contract_amount);
    let new_offer = RgbOffer::new(
        sk.to_string(),
        contract_id.clone(),
        iface.clone(),
        allocations,
        contract_amount.to_value(),
        boilerplate.precision,
        seller_address.address,
        bitcoin_price,
        seller_psbt.psbt.clone(),
        change_terminal,
        strategy,
        expire_at,
        bundle_id,
        max_claim,
    );

    Ok(new_offer)
}

pub async fn create_buyer_bid(
    sk: &str,
    request: RgbBidRequest,
) -> Result<RgbBidResponse, RgbSwapError> {
    let (mut stock, mut rgb_account) =
        retrieve_stock_account(sk).await.map_err(RgbSwapError::IO)?;

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (new_bid, resp) =
        internal_create_buyer_bid(sk, request, &mut rgb_account, &mut stock, &mut resolver).await?;

    let RgbBid {
        offer_id,
        contract_id,
        ..
    } = new_bid.clone();

    let RgbOfferSwap {
        expire_at,
        pub_key: offer_pub,
        strategy,
        ..
    } = get_public_offer(offer_id)
        .await
        .map_err(RgbSwapError::Buyer)?;

    let mut my_bids = retrieve_bids(sk).await.map_err(RgbSwapError::IO)?;
    my_bids = my_bids.save_bid(contract_id, new_bid.clone());

    store_bids(sk, my_bids).await.map_err(RgbSwapError::IO)?;
    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(RgbSwapError::IO)?;

    match strategy {
        RgbSwapStrategy::HotSwap | RgbSwapStrategy::P2P => {
            let public_bid = RgbBidSwap::from(new_bid);
            publish_swap_bid(sk, &offer_pub, public_bid.clone(), expire_at)
                .await
                .map_err(RgbSwapError::Marketplace)?;

            publish_public_bid(public_bid)
                .await
                .map_err(RgbSwapError::Marketplace)?;
        }
        invalid => return Err(RgbSwapError::WrongStrategy(invalid.to_string())),
    };

    Ok(resp)
}

pub async fn create_auction_bid(
    sk: &str,
    request: RgbAuctionBidRequest,
) -> Result<RgbAuctionBidResponse, RgbSwapError> {
    let (mut stock, mut rgb_account, mut rgb_transfers) = retrieve_stock_account_transfers(sk)
        .await
        .map_err(RgbSwapError::IO)?;

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let buyer_bid_req = RgbBidRequest::from(request.clone());
    let (mut new_bid, resp) = internal_create_buyer_bid(
        sk,
        buyer_bid_req,
        &mut rgb_account,
        &mut stock,
        &mut resolver,
    )
    .await?;

    let RgbBid {
        iface,
        contract_id,
        buyer_psbt,
        ..
    } = new_bid.clone();

    let RgbBidResponse {
        bid_id,
        offer_id,
        invoice: buyer_invoice,
        swap_psbt,
        fee_value,
    } = resp.clone();

    let RgbOfferSwap {
        strategy,
        pub_key: offer_pub,
        expire_at,
        ..
    } = get_public_offer(offer_id.clone())
        .await
        .map_err(RgbSwapError::Buyer)?;

    let mut my_bids = retrieve_bids(sk).await.map_err(RgbSwapError::IO)?;
    match strategy {
        RgbSwapStrategy::Auction | RgbSwapStrategy::Airdrop => {
            let change_terminal = match iface.to_uppercase().as_str() {
                "RGB20" => "/20/1",
                "RGB21" => "/21/1",
                _ => "/10/1",
            };

            let transfer_req = RgbTransferRequest {
                psbt: swap_psbt.clone(),
                rgb_invoice: buyer_invoice.to_string(),
                terminal: change_terminal.to_string(),
            };

            let params = NewTransferOptions {
                offer_id: Some(offer_id.clone()),
                bid_id: Some(bid_id.clone()),
                ..default!()
            };

            let RgbInternalTransferResponse {
                consig_id,
                psbt: final_psbt,
                consig: final_consig,
                outpoint,
                commit,
                amount,
                ..
            } = internal_transfer_asset(
                transfer_req,
                params,
                &mut stock,
                &mut rgb_account,
                &mut rgb_transfers,
            )
            .await
            .map_err(RgbSwapError::Transfer)?;

            let buyer_swap_psbt = if let Some(buyer_psbt) = buyer_psbt {
                let sign_req = SignPsbtRequest {
                    psbt: buyer_psbt,
                    descriptors: request.sign_keys.clone(),
                };

                let SignedPsbtResponse {
                    psbt: buyer_swap_psbt,
                    ..
                } = sign_psbt_file(sign_req)
                    .await
                    .map_err(|op| RgbSwapError::WrongPsbtBuyer(op.to_string()))?;

                Some(buyer_swap_psbt)
            } else {
                None
            };

            let sign_req = SignPsbtRequest {
                psbt: final_psbt,
                descriptors: request.sign_keys,
            };

            let SignedPsbtResponse {
                psbt: final_swap_psbt,
                ..
            } = sign_psbt_file(sign_req)
                .await
                .map_err(|op| RgbSwapError::WrongPsbtFinal(op.to_string()))?;

            new_bid.transfer_id = Some(consig_id.clone());
            new_bid.transfer = Some(final_consig.clone());
            my_bids = my_bids.save_bid(contract_id, new_bid.clone());

            let mut bid_swap = RgbBidSwap::from(new_bid);
            publish_public_bid(bid_swap.clone())
                .await
                .map_err(RgbSwapError::Marketplace)?;

            bid_swap.swap_outpoint = Some(outpoint);
            bid_swap.swap_amount = Some(amount);
            bid_swap.swap_commit = Some(commit);
            bid_swap.buyer_psbt = None;
            bid_swap.swap_psbt = None;

            publish_swap_bid(sk, &offer_pub, bid_swap.clone(), expire_at)
                .await
                .map_err(RgbSwapError::Auction)?;

            bid_swap.buyer_psbt = buyer_swap_psbt;
            bid_swap.swap_psbt = Some(final_swap_psbt.clone());
            publish_auction_bid(bid_swap)
                .await
                .map_err(RgbSwapError::Auction)?;

            store_bids(sk, my_bids).await.map_err(RgbSwapError::IO)?;
        }
        invalid => return Err(RgbSwapError::WrongStrategy(invalid.to_string())),
    };

    store_stock_account_transfers(sk, stock, rgb_account, rgb_transfers)
        .await
        .map_err(RgbSwapError::IO)?;

    let resp = RgbAuctionBidResponse {
        bid_id,
        offer_id,
        fee_value,
    };

    Ok(resp)
}

async fn internal_create_buyer_bid(
    sk: &str,
    request: RgbBidRequest,
    rgb_account: &mut RgbAccountV1,
    rgb_stock: &mut Stock,
    resolver: &mut ExplorerResolver,
) -> Result<(RgbBid, RgbBidResponse), RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let RgbBidRequest {
        offer_id,
        change_terminal,
        asset_amount: bid_amount,
        ..
    } = request.clone();

    let RgbOfferSwap {
        iface,
        contract_id,
        bitcoin_price,
        asset_precision,
        strategy,
        expire_at,
        bundle_id,
        seller_psbt,
        ..
    } = get_public_offer(offer_id.clone())
        .await
        .map_err(RgbSwapError::Buyer)?;

    let seller_psbt = match seller_psbt {
        Some(psbt) => psbt,
        None => {
            let bundle_id = bundle_id.unwrap_or_default();
            match get_auction_offer(&bundle_id, offer_id.clone())
                .await
                .map_err(RgbSwapError::Buyer)?
            {
                Some(auction_offer) => auction_offer.seller_psbt.unwrap_or_default(),
                _ => return Err(RgbSwapError::NoOffer(offer_id.to_string())),
            }
        }
    };

    let mut rgb_wallet = match rgb_account.wallets.get(RGB_DEFAULT_NAME) {
        Some(rgb_wallet) => rgb_wallet.to_owned(),
        _ => return Err(RgbSwapError::NoWatcher),
    };

    let (mut new_bid, buyer_outpoint, swap_psbt, fee_value) =
        if strategy == RgbSwapStrategy::Airdrop {
            let buyer_outpoint = watcher_next_utxo(sk, RGB_DEFAULT_NAME, &iface.to_uppercase())
                .await
                .map_err(|op| RgbSwapError::NoUtxo(op.to_string()))?;

            let buyer_outpoint = if let Some(utxo) = buyer_outpoint.utxo {
                utxo.outpoint.to_string()
            } else {
                return Err(RgbSwapError::NoUtxo(String::new()));
            };

            let bid_amount = ContractAmount::from_decimal_str(bid_amount);
            let new_bid = RgbBid::new(
                sk.to_string(),
                offer_id,
                contract_id.clone(),
                bid_amount.to_value(),
                asset_precision,
                bitcoin_price,
                vec![],
            );

            (new_bid, buyer_outpoint, seller_psbt, 0)
        } else {
            let (mut new_bid, bitcoin_inputs, bitcoin_changes, fee_value) =
                prebuild_buyer_swap(sk, request, &mut rgb_wallet, resolver).await?;
            new_bid.iface = iface.to_uppercase();

            if let Some(expire_at) = expire_at {
                let utc = chrono::Local::now().naive_utc().timestamp();
                if expire_at.sub(utc) <= 0 {
                    return Err(RgbSwapError::OfferExpired);
                }
            }

            if new_bid.bitcoin_amount.cmp(&bitcoin_price) == Ordering::Less {
                return Err(RgbSwapError::Inflation {
                    input: new_bid.bitcoin_amount,
                    output: bitcoin_price,
                });
            };

            let buyer_outpoint = watcher_next_utxo(sk, RGB_DEFAULT_NAME, &iface.to_uppercase())
                .await
                .map_err(|op| RgbSwapError::NoUtxo(op.to_string()))?;

            let buyer_outpoint = if let Some(utxo) = buyer_outpoint.utxo {
                utxo.outpoint.to_string()
            } else {
                return Err(RgbSwapError::NoUtxo(String::new()));
            };

            rgb_account
                .wallets
                .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet.clone());

            let psbt_req = PsbtRequest {
                fee: PsbtFeeRequest::Value(fee_value),
                asset_inputs: vec![],
                bitcoin_inputs,
                bitcoin_changes,
                asset_descriptor_change: None,
                asset_terminal_change: Some(change_terminal.clone()),
                rbf: true,
            };

            let options = NewPsbtOptions {
                set_tapret: false,
                ..default!()
            };

            let PsbtResponse {
                psbt: buyer_psbt, ..
            } = internal_create_psbt(psbt_req, rgb_account, resolver, Some(options))
                .await
                .map_err(RgbSwapError::Create)?;

            new_bid.buyer_psbt = Some(buyer_psbt.clone());

            let seller_psbt = Psbt::from_str(&seller_psbt)
                .map_err(|op| RgbSwapError::WrongPsbtSeller(op.to_string()))?;

            let buyer_psbt = Psbt::from_str(&buyer_psbt)
                .map_err(|op| RgbSwapError::WrongPsbtBuyer(op.to_string()))?;

            let seller_psbt = PsbtV0::from(seller_psbt);
            let buyer_psbt = PsbtV0::from(buyer_psbt);

            let swap_psbt = seller_psbt
                .join(buyer_psbt)
                .map_err(|op| RgbSwapError::WrongPsbtSwap(op.to_string()))?;

            let swap_psbt = Psbt::from(swap_psbt);
            let swap_psbt = Serialize::serialize(&swap_psbt).to_hex();

            (new_bid, buyer_outpoint, swap_psbt, fee_value)
        };

    let RgbBid {
        bid_id,
        offer_id,
        asset_amount,
        asset_precision,
        ..
    } = new_bid.clone();

    let invoice_amount = ContractAmount::new(asset_amount, asset_precision);
    let invoice_req = InvoiceRequest {
        iface,
        contract_id: contract_id.to_string(),
        amount: invoice_amount.to_string(),
        seal: format!("tapret1st:{buyer_outpoint}"),
        params: HashMap::new(),
    };
    let invoice = internal_create_invoice(invoice_req, rgb_stock)
        .await
        .map_err(RgbSwapError::Invoice)?;

    let invoice = invoice.to_string();
    new_bid.buyer_invoice = invoice.clone();

    let resp = RgbBidResponse {
        bid_id,
        offer_id,
        invoice,
        swap_psbt,
        fee_value,
    };

    Ok((new_bid, resp))
}

pub async fn create_swap_transfer(
    sk: &str,
    request: RgbSwapRequest,
) -> Result<RgbSwapResponse, RgbSwapError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(RgbSwapError::Validation(errors));
    }

    let (mut stock, mut rgb_account, mut rgb_transfers) = retrieve_stock_account_transfers(sk)
        .await
        .map_err(RgbSwapError::IO)?;

    let RgbSwapRequest {
        offer_id,
        bid_id,
        swap_psbt,
        ..
    } = request.clone();

    let RgbOfferSwap {
        iface,
        expire_at,
        pub_key: offer_pub,
        strategy,
        ..
    } = get_public_offer(offer_id.clone())
        .await
        .map_err(RgbSwapError::Swap)?;

    let mut rgb_swap_bid = match strategy {
        RgbSwapStrategy::P2P | RgbSwapStrategy::Auction => {
            get_swap_bid_by_buyer(sk, offer_id.clone(), bid_id.clone())
                .await
                .map_err(RgbSwapError::Swap)?
        }
        _ => get_swap_bid_by_seller(sk, offer_id.clone(), bid_id.clone(), expire_at)
            .await
            .map_err(RgbSwapError::Swap)?,
    };

    let RgbBidSwap {
        contract_id,
        buyer_invoice,
        pub_key: buyer_pub,
        ..
    } = rgb_swap_bid.clone();
    let change_terminal = match iface.to_uppercase().as_str() {
        "RGB20" => "/20/1",
        "RGB21" => "/21/1",
        _ => "/10/1",
    };

    let transfer_req = RgbTransferRequest {
        psbt: swap_psbt.clone(),
        rgb_invoice: buyer_invoice.to_string(),
        terminal: change_terminal.to_string(),
    };

    let params = NewTransferOptions {
        offer_id: Some(offer_id.clone()),
        bid_id: Some(bid_id.clone()),
        ..default!()
    };

    let RgbInternalTransferResponse {
        consig_id,
        psbt: final_psbt,
        consig: final_consig,
        outpoint,
        commit,
        amount,
        ..
    } = internal_transfer_asset(
        transfer_req,
        params,
        &mut stock,
        &mut rgb_account,
        &mut rgb_transfers,
    )
    .await
    .map_err(RgbSwapError::Transfer)?;

    let counter_party = match strategy {
        RgbSwapStrategy::P2P => {
            let mut my_bids = retrieve_bids(sk).await.map_err(RgbSwapError::IO)?;
            update_transfer_bid(bid_id.clone(), consig_id.clone(), &mut my_bids)
                .await
                .map_err(RgbSwapError::Swap)?;

            store_bids(sk, my_bids).await.map_err(RgbSwapError::IO)?;

            rgb_swap_bid.swap_outpoint = Some(outpoint);
            rgb_swap_bid.swap_amount = Some(amount);
            rgb_swap_bid.swap_commit = Some(commit);
            offer_pub
        }
        RgbSwapStrategy::HotSwap => {
            let mut my_offers = retrieve_offers(sk).await.map_err(RgbSwapError::IO)?;
            update_transfer_offer(offer_id.clone(), consig_id.clone(), &mut my_offers)
                .await
                .map_err(RgbSwapError::Swap)?;

            store_offers(sk, my_offers.clone())
                .await
                .map_err(RgbSwapError::IO)?;

            if let Some(list_offers) = my_offers.clone().offers.get(&contract_id) {
                if let Some(my_offer) = list_offers.iter().find(|x| x.offer_id == offer_id) {
                    let mut rgb_wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME).unwrap().clone();
                    save_rgb_commit_str(
                        &outpoint,
                        amount,
                        &commit,
                        &my_offer.terminal,
                        &mut rgb_wallet,
                    );
                    rgb_account
                        .wallets
                        .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet);
                }
            }
            buyer_pub
        }
        invalid => return Err(RgbSwapError::WrongStrategy(invalid.to_string())),
    };

    let RgbExtractTransfer { strict, .. } = prebuild_extract_transfer(&final_consig)
        .map_err(|op| RgbSwapError::WrongConsigSwap(op.to_string()))?;
    rgb_swap_bid.transfer_id = Some(consig_id.clone());
    rgb_swap_bid.transfer = Some(strict.to_hex());
    rgb_swap_bid.swap_psbt = Some(final_psbt.clone());

    publish_swap_bid(sk, &counter_party, rgb_swap_bid, expire_at)
        .await
        .map_err(RgbSwapError::Swap)?;

    store_stock_account_transfers(sk, stock, rgb_account, rgb_transfers)
        .await
        .map_err(RgbSwapError::IO)?;

    Ok(RgbSwapResponse {
        consig_id,
        final_consig,
        final_psbt,
    })
}

pub async fn direct_swap_transfer(
    sk: &str,
    request: RgbBidRequest,
) -> Result<RgbSwapResponse, RgbSwapError> {
    let bid_response = create_buyer_bid(sk, request.clone()).await?;
    create_swap_transfer(
        sk,
        RgbSwapRequest {
            offer_id: request.offer_id,
            bid_id: bid_response.bid_id,
            swap_psbt: bid_response.swap_psbt,
        },
    )
    .await
}

pub async fn internal_replace_transfer(
    sk: &str,
    request: RgbTransferRequest,
    options: NewTransferOptions,
) -> Result<RgbReplaceResponse, TransferError> {
    let (mut stock, mut rgb_account, mut rgb_transfers) = retrieve_stock_account_transfers(sk)
        .await
        .map_err(TransferError::IO)?;

    let RgbInternalTransferResponse {
        consig_id,
        consig,
        psbt,
        commit,
        outpoint,
        consigs,
        amount,
        txid,
        ..
    } = internal_transfer_asset(
        request.clone(),
        options,
        &mut stock,
        &mut rgb_account,
        &mut rgb_transfers,
    )
    .await?;

    let mut rgb_wallet = match rgb_account.wallets.get(RGB_DEFAULT_NAME) {
        Some(rgb_wallet) => rgb_wallet.to_owned(),
        _ => return Err(TransferError::NoWatcher),
    };

    save_rgb_commit_str(
        &outpoint,
        amount,
        &commit,
        &request.terminal,
        &mut rgb_wallet,
    );
    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet);

    let resp = RgbReplaceResponse {
        consig_id,
        consig,
        psbt,
        commit,
        consigs,
        txid,
    };

    store_stock_account_transfers(sk, stock, rgb_account, rgb_transfers)
        .await
        .map_err(TransferError::IO)?;

    Ok(resp)
}

pub async fn finish_auction_offers(
    sk: &str,
    bundle_id: String,
) -> Result<RgbAuctionFinishResponse, RgbSwapError> {
    let (mut stock, mut rgb_account, mut rgb_transfers) = retrieve_stock_account_transfers(sk)
        .await
        .map_err(RgbSwapError::IO)?;

    let my_offers = retrieve_offers(sk).await.map_err(RgbSwapError::IO)?;
    let auction = get_auction(&bundle_id.to_string())
        .await
        .map_err(RgbSwapError::Auction)?;

    if auction.is_none() || my_offers.clone().get_offers(bundle_id.clone()).is_empty() {
        return Err(RgbSwapError::NoBundle(bundle_id));
    }

    let auction = auction.unwrap_or_default();
    let offers = my_offers.get_offers(bundle_id.clone());
    let bids = match auction.strategy {
        RgbAuctionStrategy::Auction => get_auction_highest_bids(bundle_id.clone())
            .await
            .map_err(RgbSwapError::Auction)?,
        RgbAuctionStrategy::Airdrop { max_claim: _ } => get_auction_fifo_bids(bundle_id.clone())
            .await
            .map_err(RgbSwapError::Auction)?,
    };

    let mut final_psbt: Option<PsbtV0> = none!();
    for bid in bids.clone() {
        if let Some(offer) = offers
            .clone()
            .into_iter()
            .find(|x| x.offer_id == bid.offer_id)
        {
            let seller_part = Psbt::from_str(&offer.seller_psbt)
                .map_err(|op| RgbSwapError::WrongPsbtSeller(op.to_string()))?;

            let swap_part = if let Some(buyer_psbt) = bid.buyer_psbt {
                let buyer_part = Psbt::from_str(&buyer_psbt)
                    .map_err(|op| RgbSwapError::WrongPsbtBuyer(op.to_string()))?;

                let seller_part = PsbtV0::from(seller_part);
                let buyer_part = PsbtV0::from(buyer_part);

                seller_part
                    .join(buyer_part)
                    .map_err(|op| RgbSwapError::WrongPsbtFinal(op.to_string()))?
            } else {
                PsbtV0::from(seller_part)
            };

            final_psbt = if let Some(final_psbt) = final_psbt {
                let final_psbt = final_psbt
                    .join(swap_part.clone())
                    .map_err(|op| RgbSwapError::WrongPsbtFinal(op.to_string()))?;

                Some(final_psbt)
            } else {
                Some(swap_part)
            }
        }
    }

    let mut resp = RgbAuctionFinishResponse {
        bundle_id: bundle_id.clone(),
        ..Default::default()
    };
    if let Some(RgbBidSwap {
        iface,
        buyer_invoice,
        ..
    }) = bids.clone().get(0)
    {
        let all_invoices = bids
            .clone()
            .into_iter()
            .skip(1)
            .map(|x| RgbInvoice::from_str(&x.buyer_invoice).expect(""))
            .collect();

        let change_terminal = match iface.to_uppercase().as_str() {
            "RGB20" => "/20/1",
            "RGB21" => "/21/1",
            _ => "/10/1",
        };

        let options = NewTransferOptions::with(true, all_invoices);
        let final_psbt = Psbt::from(final_psbt.unwrap());
        let final_psbt = Serialize::serialize(&final_psbt).to_hex();
        let request = RgbTransferRequest {
            psbt: final_psbt,
            rgb_invoice: buyer_invoice.clone(),
            terminal: change_terminal.to_string(),
        };

        let RgbInternalTransferResponse {
            consig_id,
            psbt: final_psbt,
            consig: final_consig,
            consigs,
            commit,
            outpoint,
            ..
        } = internal_transfer_asset(
            request,
            options.clone(),
            &mut stock,
            &mut rgb_account,
            &mut rgb_transfers,
        )
        .await
        .map_err(RgbSwapError::Transfer)?;

        publish_psbt_file(PublishPsbtRequest {
            psbt: final_psbt.clone(),
        })
        .await
        .map_err(|op| RgbSwapError::WrongPsbtFinal(op.to_string()))?;

        resp.outpoint = outpoint.clone();
        for mut bid in bids.clone() {
            let RgbBidSwap {
                pub_key: counter_party_key,
                buyer_invoice,
                ..
            } = bid.clone();
            if let Some(RgbOffer { expire_at, .. }) = offers
                .clone()
                .into_iter()
                .find(|x| x.offer_id == bid.offer_id)
            {
                let RgbInvoice { beneficiary, .. } =
                    RgbInvoice::from_str(&buyer_invoice).expect("invalid invoice");

                if let Some(strict_consig) = consigs.get(&beneficiary.to_string()) {
                    let (_, consig) = extract_transfer(strict_consig.clone()).expect("");
                    bid.transfer_id = Some(consig.bindle_id().to_string());
                    bid.transfer = Some(strict_consig.clone());
                } else {
                    bid.transfer_id = Some(consig_id.clone());
                    bid.transfer = Some(final_consig.clone());
                }

                bid.swap_outpoint = Some(outpoint.clone());
                bid.swap_commit = Some(commit.clone());

                publish_swap_bid(sk, &counter_party_key, bid, expire_at)
                    .await
                    .map_err(RgbSwapError::Swap)?;
            }
        }

        let final_psbt = Psbt::from_str(&final_psbt)
            .map_err(|op| RgbSwapError::WrongPsbtFinal(op.to_string()))?;

        let internal_request = RgbInternalSaveTransferRequest::with(
            consig_id.clone(),
            final_consig.clone(),
            buyer_invoice.to_string(),
            iface.to_string(),
            true,
            Some(consigs.clone()),
            Some(final_psbt),
        );

        internal_save_transfer(internal_request, &mut rgb_transfers)
            .await
            .map_err(|op| RgbSwapError::WrongConsigSwap(op.to_string()))?;
    }

    store_stock_account_transfers(sk, stock, rgb_account, rgb_transfers)
        .await
        .map_err(RgbSwapError::IO)?;

    // Retrieve Auctions Results
    let mut sold_items = bmap! {};
    let mut remaining_items = bmap! {};
    for RgbOffer {
        contract_id: offer_contract,
        iface: offer_iface,
        offer_id,
        asset_amount: offer_amount,
        asset_precision: offer_precision,
        ..
    } in offers.into_iter().clone()
    {
        if let Some(bid) = bids.clone().into_iter().find(|x| x.offer_id == offer_id) {
            let remaining = offer_amount - bid.asset_amount;
            let contract_amount =
                ContractAmount::new(bid.asset_amount, bid.asset_precision).to_string();
            sold_items.insert(
                offer_id.clone(),
                RgbSwapItem {
                    contract_id: offer_contract.clone(),
                    iface: offer_iface.clone(),
                    contract_amount,
                },
            );

            if remaining > 0 {
                let contract_amount =
                    ContractAmount::new(offer_amount, offer_precision).to_string();
                remaining_items.insert(
                    offer_id.clone(),
                    RgbSwapItem {
                        contract_id: offer_contract,
                        iface: offer_iface,
                        contract_amount,
                    },
                );
            }
        } else {
            let contract_amount = ContractAmount::new(offer_amount, offer_precision).to_string();
            remaining_items.insert(
                offer_id,
                RgbSwapItem {
                    contract_id: offer_contract,
                    iface: offer_iface,
                    contract_amount,
                },
            );
        }
    }

    resp.remaining = remaining_items;
    resp.sold = sold_items;

    Ok(resp)
}

async fn internal_transfer_asset(
    request: RgbTransferRequest,
    options: NewTransferOptions,
    stock: &mut Stock,
    rgb_account: &mut RgbAccountV1,
    rgb_transfers: &mut RgbTransfersV1,
) -> Result<RgbInternalTransferResponse, TransferError> {
    let network = NETWORK.read().await.to_string();
    let context = RGBContext::with(&network);

    if let Err(err) = request.validate(&context) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(TransferError::Validation(errors));
    }

    if rgb_account.wallets.get(RGB_DEFAULT_NAME).is_none() {
        return Err(TransferError::NoWatcher);
    }

    let RgbTransferRequest {
        rgb_invoice: invoice,
        psbt,
        ..
    } = request;

    let (psbt, mut transfers) =
        pay_invoice(invoice.clone(), psbt, options.clone(), stock).map_err(TransferError::Pay)?;
    let (outpoint, amount, commit) =
        extract_output_commit(psbt.clone()).map_err(TransferError::Commitment)?;

    let transfer = transfers.remove(0);
    let consig_id = transfer.bindle_id().to_string();
    let consig = transfer
        .to_strict_serialized::<{ U32 }>()
        .map_err(|err| TransferError::WrongConsig(err.to_string()))?;

    let rgb_invoice = RgbInvoice::from_str(&invoice)
        .map_err(|err| TransferError::WrongInvoice(err.to_string()))?;

    let consig = consig.to_hex();
    let commit = commit.to_hex();
    let psbt_hex = psbt.to_string();

    let iface = rgb_invoice.clone().iface.unwrap().to_string();
    let mut consigs = BTreeMap::default();

    // TODO: Make strict option check
    for prev_invoice in options.other_invoices {
        let invoice_contract = prev_invoice.contract.unwrap();
        if let Some(transfer) = transfers
            .clone()
            .into_iter()
            .find(|x| x.contract_id() == invoice_contract)
        {
            let current_transfer = transfer
                .to_strict_serialized::<{ U32 }>()
                .map_err(|err| TransferError::WrongConsig(err.to_string()))?;

            let current_transfer = current_transfer.to_hex();
            consigs.insert(prev_invoice.beneficiary.to_string(), current_transfer);
        }
    }

    let internal_request = RgbInternalSaveTransferRequest::with(
        consig_id.clone(),
        consig.clone(),
        rgb_invoice.beneficiary.to_string(),
        iface,
        true,
        Some(consigs.clone()),
        Some(psbt),
    );

    let txid = internal_save_transfer(internal_request, rgb_transfers)
        .await
        .map_err(TransferError::WrongSave)?;

    let resp = RgbInternalTransferResponse {
        consig_id,
        consig,
        amount,
        psbt: psbt_hex,
        commit,
        outpoint: outpoint.to_string(),
        consigs,
        txid: txid.to_hex(),
    };

    Ok(resp)
}

pub async fn list_auctions() -> Result<Vec<RgbAuctionOfferResponse>, RgbSwapError> {
    let utc = chrono::Local::now().naive_utc().timestamp();
    let auction_offers: Vec<_> = get_public_offers()
        .await
        .map_err(RgbSwapError::Auction)?
        .into_iter()
        .filter(|x| {
            x.strategy == RgbSwapStrategy::Auction
                && x.bundle_id.is_some()
                && x.expire_at.unwrap_or_default().sub(utc) <= 0
        })
        .map(RgbAuctionOfferResponse::from)
        .collect();

    Ok(auction_offers)
}

pub async fn accept_transfer(
    sk: &str,
    request: AcceptRequest,
) -> Result<AcceptResponse, TransferError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(TransferError::Validation(errors));
    }
    let mut stock = retrieve_rgb_stock(sk).await.map_err(TransferError::IO)?;
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let AcceptRequest { consignment, .. } = request;
    prefetch_resolver_rgb(&consignment, &mut resolver, None).await;

    let transfer = accept_rgb_transfer(consignment, false, &mut resolver, &mut stock)
        .map_err(TransferError::Accept)?;

    let resp = AcceptResponse {
        contract_id: transfer.contract_id().to_string(),
        transfer_id: transfer.transfer_id().to_string(),
        valid: true,
    };

    store_rgb_stock(sk, stock)
        .await
        .map_err(TransferError::IO)?;

    Ok(resp)
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum SaveTransferError {
    /// Some request data is missing. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(RgbPersistenceError),
    /// Proxy connectivity error. {0}
    Proxy(ProxyError),
    /// Occurs an error in parse swap psbt step. {0}
    WrongPsbt(String),
    /// Occurs an error in parse consig step. {0}
    WrongConsig(AcceptTransferError),
    /// Occurs an error in parse consig swap step. {0}
    WrongConsigSwap(AcceptTransferError),
    /// Occurs an error in parse invoice step. {0}
    WrongInvoice(String),
    /// Occurs an error in swap step. {0}
    WrongSwap(RgbOfferErrors),
    /// Write I/O or connectivity error. {1} in {0}
    Write(String, String),
}

pub async fn save_transfer(
    sk: &str,
    request: RgbSaveTransferRequest,
) -> Result<RgbTransferStatusResponse, SaveTransferError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(SaveTransferError::Validation(errors));
    }

    let RgbSaveTransferRequest { iface, consignment } = request;

    let mut rgb_transfers = retrieve_transfers(sk)
        .await
        .map_err(SaveTransferError::IO)?;

    let RgbExtractTransfer {
        consig_id,
        contract_id,
        ..
    } = prebuild_extract_transfer(&consignment)?;

    let request = RgbInternalSaveTransferRequest::with(
        consig_id.clone(),
        consignment,
        String::new(),
        iface,
        false,
        None,
        None,
    );

    internal_save_transfer(request, &mut rgb_transfers).await?;

    let mut status = BTreeMap::new();
    status.insert(consig_id.clone(), false);

    store_transfers(sk, rgb_transfers)
        .await
        .map_err(SaveTransferError::IO)?;

    Ok(RgbTransferStatusResponse {
        contract_id,
        consig_status: status,
    })
}

pub async fn internal_save_transfer(
    request: RgbInternalSaveTransferRequest,
    rgb_transfers: &mut RgbTransfersV1,
) -> Result<rgb::Txid, SaveTransferError> {
    let RgbInternalSaveTransferRequest {
        iface,
        consig: consignment,
        sender,
        utxos,
        beneficiaries,
        ..
    } = request;

    let RgbExtractTransfer {
        consig_id,
        contract_id,
        txid: tx_id,
        strict,
        ..
    } = prebuild_extract_transfer(&consignment)?;

    let beneficiaries = if let Some(beneficiaries) = beneficiaries {
        beneficiaries
    } else {
        BTreeMap::new()
    };

    let secret_seals: Vec<String> = beneficiaries.keys().map(|x| x.to_string()).collect();

    let consig = strict.to_hex();
    let rgb_transfer = RgbTransferV1 {
        consig_id: consig_id.clone(),
        consig: consig.clone(),
        iface,
        tx_id,
        sender,
        utxos,
        beneficiaries: secret_seals,
        rbf: true,
    };

    if let Some(transfers) = rgb_transfers.transfers.get(&contract_id.clone()) {
        let mut current_transfers = transfers.clone();

        if let Some(pos) = current_transfers
            .clone()
            .into_iter()
            .position(|x| x.consig_id == consig_id)
        {
            current_transfers.remove(pos);
            current_transfers.insert(pos, rgb_transfer);
        } else {
            current_transfers.push(rgb_transfer);
        }
        rgb_transfers
            .transfers
            .insert(contract_id.clone(), current_transfers.to_vec());
    } else {
        rgb_transfers
            .transfers
            .insert(contract_id.clone(), vec![rgb_transfer]);
    }

    post_consignments(beneficiaries)
        .await
        .map_err(SaveTransferError::Proxy)?;

    Ok(tx_id)
}

pub async fn remove_transfer(
    sk: &str,
    request: RgbRemoveTransferRequest,
) -> Result<RgbTransferStatusResponse, SaveTransferError> {
    if let Err(err) = request.validate(&RGBContext::default()) {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(SaveTransferError::Validation(errors));
    }

    let RgbRemoveTransferRequest {
        contract_id,
        consig_ids,
    } = request;

    let mut rgb_transfers = retrieve_transfers(sk)
        .await
        .map_err(SaveTransferError::IO)?;

    if let Some(transfers) = rgb_transfers.transfers.get(&contract_id.clone()) {
        let current_transfers = transfers
            .clone()
            .into_iter()
            .filter(|x| !consig_ids.contains(&x.consig_id))
            .collect();

        rgb_transfers
            .transfers
            .insert(contract_id.clone(), current_transfers);
    }

    store_transfers(sk, rgb_transfers)
        .await
        .map_err(SaveTransferError::IO)?;

    let status = consig_ids.into_iter().map(|x| (x, true)).collect();
    Ok(RgbTransferStatusResponse {
        contract_id,
        consig_status: status,
    })
}

pub async fn verify_transfers(sk: &str) -> Result<BatchRgbTransferResponse, TransferError> {
    let (mut stock, mut rgb_accounts, mut rgb_transfers) = retrieve_stock_account_transfers(sk)
        .await
        .map_err(TransferError::IO)?;

    internal_extract_transfers_seals(rgb_accounts.clone(), &mut rgb_transfers).await?;
    internal_extract_transfers_swaps(sk, &mut rgb_accounts, &mut rgb_transfers)
        .await
        .map_err(TransferError::Save)?;

    let (rgb_pending, transfers) = internal_verify_transfers(&mut stock, rgb_transfers).await?;

    let mut my_public_offers = vec![];
    let check_offers: Vec<_> = transfers
        .clone()
        .into_iter()
        .filter(|x| x.is_mine)
        .map(|x| x.consig_id)
        .collect();

    let check_bids: Vec<_> = transfers
        .clone()
        .into_iter()
        .filter(|x| x.is_mine)
        .map(|x| x.consig_id)
        .collect();

    if !check_offers.is_empty() {
        let mut my_offers = retrieve_offers(sk).await.map_err(TransferError::IO)?;
        for transfer_id in check_offers {
            if let Some(offer) = complete_offer(transfer_id, &mut my_offers)
                .await
                .map_err(TransferError::WrongSwap)?
            {
                my_public_offers.push(offer);
            }
        }
        store_offers(sk, my_offers)
            .await
            .map_err(TransferError::IO)?;
    }

    if !check_bids.is_empty() {
        let mut my_bids = retrieve_bids(sk).await.map_err(TransferError::IO)?;
        for transfer_id in check_bids {
            complete_bid(transfer_id, &mut my_bids)
                .await
                .map_err(TransferError::WrongSwap)?;
        }
        store_bids(sk, my_bids).await.map_err(TransferError::IO)?;
    }

    if !my_public_offers.is_empty() {
        remove_public_offers(my_public_offers)
            .await
            .map_err(TransferError::WrongSwap)?;
    }

    store_stock_account_transfers(sk, stock, rgb_accounts, rgb_pending)
        .await
        .map_err(TransferError::IO)?;

    Ok(BatchRgbTransferResponse { transfers })
}

pub async fn internal_extract_transfers_swaps(
    sk: &str,
    rgb_accounts: &mut RgbAccountV1,
    rgb_transfers: &mut RgbTransfersV1,
) -> Result<(), SaveTransferError> {
    let mut my_swaps = vec![];
    let mut current_offers = vec![];
    let my_offers = retrieve_offers(sk).await.map_err(SaveTransferError::IO)?;
    my_offers
        .offers
        .values()
        .for_each(|bs| current_offers.extend(bs.to_owned()));

    let mut rgb_wallet = rgb_accounts.wallets.get(RGB_DEFAULT_NAME).unwrap().clone();
    current_offers.retain(|x| !x.clone().is_fill());
    for offer in current_offers {
        if let Ok(swaps_bids) = get_swap_bids_by_offer(sk, offer.clone()).await {
            my_swaps.extend(swaps_bids.clone());

            for swap_bid in swaps_bids {
                let RgbBidSwap {
                    swap_commit,
                    swap_outpoint,
                    swap_amount,
                    ..
                } = swap_bid;
                if swap_commit.is_some() && swap_outpoint.is_some() {
                    save_rgb_commit_str(
                        &swap_outpoint.unwrap_or_default(),
                        swap_amount.unwrap_or_default(),
                        &swap_commit.unwrap_or_default(),
                        &offer.terminal,
                        &mut rgb_wallet,
                    );
                }
            }
        }
    }
    rgb_accounts
        .wallets
        .insert(RGB_DEFAULT_NAME.to_owned(), rgb_wallet);

    let my_bids = retrieve_bids(sk).await.map_err(SaveTransferError::IO)?;
    let mut current_bids = vec![];
    my_bids
        .bids
        .values()
        .for_each(|bs| current_bids.extend(bs.to_owned()));

    current_bids.retain(|x| !x.clone().is_fill());
    for bid in current_bids {
        if let Ok(swaps_bid) =
            get_swap_bid_by_buyer(sk, bid.offer_id.clone(), bid.bid_id.clone()).await
        {
            my_swaps.push(swaps_bid);
        }
    }

    for RgbBidSwap {
        iface,
        buyer_invoice,
        transfer_id,
        transfer,
        swap_psbt,
        ..
    } in my_swaps
    {
        if let Some(transfer) = transfer {
            let psbt = if let Some(psbt) = swap_psbt {
                Some(
                    Psbt::from_str(&psbt)
                        .map_err(|op| SaveTransferError::WrongPsbt(op.to_string()))?,
                )
            } else {
                None
            };

            let invoice = RgbInvoice::from_str(&buyer_invoice)
                .map_err(|op| SaveTransferError::WrongInvoice(op.to_string()))?;

            let request = RgbInternalSaveTransferRequest::with(
                transfer_id.unwrap_or_default(),
                transfer,
                invoice.beneficiary.to_string(),
                iface,
                true,
                None,
                psbt,
            );

            internal_save_transfer(request, rgb_transfers).await?;
        }
    }

    Ok(())
}

pub async fn internal_extract_transfers_seals(
    rgb_account: RgbAccountV1,
    rgb_transfers: &mut RgbTransfersV1,
) -> Result<Vec<RgbTransferV1>, TransferError> {
    let mut all_transfers = vec![];
    rgb_transfers
        .transfers
        .values()
        .for_each(|f| all_transfers.extend(f));

    all_transfers.retain(|x| !x.sender);

    let mut retrieve_by_seal = BTreeMap::new();
    for invoice in rgb_account.invoices {
        let rgb_invoice = RgbInvoice::from_str(&invoice)
            .map_err(|op| TransferError::WrongInvoice(op.to_string()))?;
        let seal = rgb_invoice.beneficiary.to_string();
        if all_transfers
            .clone()
            .into_iter()
            .any(|x| x.beneficiaries.contains(&seal))
        {
            continue;
        }

        let iface = if let Some(iface) = rgb_invoice.iface {
            iface.to_string()
        } else {
            String::new()
        };

        retrieve_by_seal.insert(seal, iface);
    }

    let mut new_transfers = vec![];
    for (seal, iface) in retrieve_by_seal {
        if let Some(transfer) = get_rgb_consignment(&seal)
            .await
            .map_err(TransferError::Proxy)?
        {
            let (tx_id, transfer) = extract_transfer(transfer)
                .map_err(|op| TransferError::WrongConsig(op.to_string()))?;

            let consig_id = transfer.id().to_string();
            let contract_id = transfer.contract_id().to_string();
            let consig = transfer
                .unbindle()
                .to_strict_serialized::<{ U32 }>()
                .map_err(|op| TransferError::WrongConsig(op.to_string()))?
                .to_hex();

            let rgb_transfer =
                RgbTransferV1::new(consig_id.clone(), consig, iface, tx_id, vec![seal]);
            new_transfers.push(rgb_transfer.clone());

            if let Some(transfers) = rgb_transfers.transfers.get(&contract_id.clone()) {
                let mut current_transfers = transfers.clone();

                if let Some(pos) = current_transfers
                    .clone()
                    .into_iter()
                    .position(|x| x.consig_id == consig_id)
                {
                    current_transfers.remove(pos);
                    current_transfers.insert(pos, rgb_transfer);
                } else {
                    current_transfers.push(rgb_transfer);
                }
                rgb_transfers
                    .transfers
                    .insert(contract_id.clone(), current_transfers.to_vec());
            } else {
                rgb_transfers
                    .transfers
                    .insert(contract_id.clone(), vec![rgb_transfer]);
            }
        }
    }

    Ok(new_transfers)
}

pub async fn internal_verify_transfers(
    stock: &mut Stock,
    rgb_transfers: RgbTransfersV1,
) -> Result<(RgbTransfersV1, Vec<BatchRgbTransferItem>), TransferError> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let mut transfers = vec![];
    let mut rgb_pending = RgbTransfersV1::default();
    for (contract_id, transfer_activities) in rgb_transfers.transfers {
        let mut pending_transfers = vec![];
        let txids: Vec<bitcoin::Txid> = transfer_activities
            .clone()
            .into_iter()
            .map(|x| Txid::from_str(&x.tx_id.to_hex()).expect("invalid tx id"))
            .collect();
        prefetch_resolver_txs_status(txids, &mut resolver).await;

        for activity in transfer_activities {
            let iface = activity.iface.clone();
            let txid = Txid::from_str(&activity.tx_id.to_hex()).expect("invalid tx id");
            let status = resolver
                .txs_status
                .get(&txid)
                .unwrap_or(&TxStatus::NotFound)
                .to_owned();

            let accept_status = match status.clone() {
                TxStatus::Block(_) => {
                    prefetch_resolver_rgb(&activity.consig, &mut resolver, None).await;
                    accept_rgb_transfer(activity.consig.clone(), false, &mut resolver, stock)
                        .map_err(TransferError::Accept)?
                }
                _ => {
                    pending_transfers.push(activity.to_owned());
                    transfers.push(BatchRgbTransferItem {
                        iface,
                        status,
                        is_accept: false,
                        contract_id: contract_id.clone(),
                        consig_id: activity.consig_id.to_string(),
                        is_mine: activity.sender,
                        txid: txid.to_hex(),
                    });
                    continue;
                }
            };
            let transfer_id = accept_status.transfer_id();
            let accept_status = accept_status.unbindle();
            if let Some(rgb_status) = accept_status.into_validation_status() {
                if rgb_status.validity() == Validity::Valid {
                    transfers.push(BatchRgbTransferItem {
                        iface,
                        status,
                        is_accept: true,
                        contract_id: contract_id.clone(),
                        consig_id: transfer_id.to_string(),
                        is_mine: activity.sender,
                        txid: txid.to_hex(),
                    });
                } else {
                    transfers.push(BatchRgbTransferItem {
                        iface,
                        status,
                        is_accept: false,
                        contract_id: contract_id.clone(),
                        consig_id: transfer_id.to_string(),
                        is_mine: activity.sender,
                        txid: txid.to_hex(),
                    });
                    pending_transfers.push(activity.to_owned());
                }
            }
        }

        rgb_pending
            .transfers
            .insert(contract_id.to_string(), pending_transfers);
    }

    Ok((rgb_pending, transfers))
}

pub async fn get_contract(sk: &str, contract_id: &str) -> Result<ContractResponse> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) = retrieve_stock_account(sk).await?;

    let contract_id = ContractId::from_str(contract_id)?;
    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                let contract_index = contract_type.clone() as u32;
                let iface_name = contract_type.to_string().to_uppercase().clone();

                let iface = stock
                    .iface_by_name(&tn!(iface_name.clone()))
                    .map_err(|_| TransferError::NoIface)?;

                if let Ok(contract_iface) = stock.contract_iface(contract_id, iface.iface_id()) {
                    sync_wallet(contract_index, &mut fetch_wallet, &mut resolver);
                    prefetch_resolver_allocations(contract_iface, &mut resolver, true).await;
                    prefetch_resolver_utxos(
                        contract_index,
                        &mut fetch_wallet,
                        &mut resolver,
                        Some(RGB_DEFAULT_FETCH_LIMIT),
                    )
                    .await;
                }
            }

            Some(fetch_wallet)
        }
        _ => None,
    };

    let mut contract = export_contract(contract_id, &mut stock, &mut resolver, &mut wallet)?;
    contract.meta = if let Some(meta) = contract.meta {
        Some(
            extract_metadata(meta)
                .await
                .expect("Error to retrieve metadata"),
        )
    } else {
        None
    };

    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);
        store_account(sk, rgb_account).await?;
    };

    Ok(contract)
}

pub async fn get_simple_contract(sk: &str, contract_id: &str) -> Result<SimpleContractResponse> {
    let mut stock = retrieve_rgb_stock(sk).await?;
    let contract_id = ContractId::from_str(contract_id)?;
    let contract = export_boilerplate(contract_id, &mut stock)?;

    let ContractBoilerplate {
        contract_id,
        iface_id,
        precision,
    } = contract;

    Ok(SimpleContractResponse {
        contract_id,
        iface_id,
        precision,
    })
}

pub async fn hidden_contract(sk: &str, contract_id: &str) -> Result<ContractHiddenResponse> {
    let mut rgb_account = retrieve_account(sk).await?;
    if !rgb_account
        .hidden_contracts
        .contains(&contract_id.to_string())
    {
        rgb_account.hidden_contracts.push(contract_id.to_string());
        store_account(sk, rgb_account).await?;
    }

    Ok(ContractHiddenResponse {
        contract_id: contract_id.to_string(),
        hidden: true,
    })
}

pub async fn list_contracts(sk: &str, hidden_contracts: bool) -> Result<ContractsResponse> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) = retrieve_stock_account(sk).await?;

    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                let contract_index = contract_type as u32;
                sync_wallet(contract_index, &mut fetch_wallet, &mut resolver);
                prefetch_resolver_utxos(
                    contract_index,
                    &mut fetch_wallet,
                    &mut resolver,
                    Some(RGB_DEFAULT_FETCH_LIMIT),
                )
                .await;
            }
            Some(fetch_wallet)
        }
        _ => None,
    };

    let mut contracts = vec![];
    for contract_type in [AssetType::RGB20, AssetType::RGB21] {
        let iface_name = contract_type.to_string().to_uppercase().clone();
        let iface_name = tn!(iface_name);
        let iface = stock
            .iface_by_name(&iface_name)
            .expect("Iface name not found")
            .clone();

        let contract_ids = stock
            .contract_ids_by_iface(&iface_name)
            .expect("contract not found");

        for contract_id in contract_ids {
            if hidden_contracts
                && rgb_account
                    .hidden_contracts
                    .contains(&contract_id.to_string())
            {
                continue;
            }

            let contract_iface = stock
                .clone()
                .contract_iface(contract_id, iface.iface_id())
                .expect("Iface not found");

            prefetch_resolver_allocations(contract_iface, &mut resolver, true).await;
            let mut resp = export_contract(contract_id, &mut stock, &mut resolver, &mut wallet)?;
            resp.meta = if let Some(meta) = resp.meta {
                Some(
                    extract_metadata(meta)
                        .await
                        .expect("Error to retrieve metadata"),
                )
            } else {
                None
            };
            contracts.push(resp);
        }
    }

    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);
        store_account(sk, rgb_account).await?;
    };

    Ok(ContractsResponse { contracts })
}

pub async fn list_interfaces(sk: &str) -> Result<InterfacesResponse> {
    let stock = retrieve_rgb_stock(sk).await?;

    let mut interfaces = vec![];
    for schema_id in stock.schema_ids()? {
        let schema = stock.schema(schema_id)?;
        for (iface_id, iimpl) in schema.clone().iimpls.into_iter() {
            let face = stock.iface_by_id(iface_id)?;

            let item = InterfaceDetail {
                name: face.name.to_string(),
                iface: iface_id.to_string(),
                iimpl: iimpl.impl_id().to_string(),
            };
            interfaces.push(item)
        }
    }

    Ok(InterfacesResponse { interfaces })
}

pub async fn list_schemas(sk: &str) -> Result<SchemasResponse> {
    let stock = retrieve_rgb_stock(sk).await?;

    let mut schemas = vec![];
    for schema_id in stock.schema_ids()? {
        let schema = stock.schema(schema_id)?;
        let mut ifaces = vec![];
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            let face = stock.iface_by_id(iface_id)?;
            ifaces.push(face.name.to_string());
        }
        schemas.push(SchemaDetail {
            schema: schema_id.to_string(),
            ifaces,
        })
    }

    Ok(SchemasResponse { schemas })
}

pub async fn list_transfers(sk: &str, contract_id: String) -> Result<RgbTransfersResponse> {
    let rgb_transfers = retrieve_transfers(sk).await?;

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let mut transfers = vec![];
    if let Some(transfer_activities) = rgb_transfers.transfers.get(&contract_id) {
        let transfer_activities = transfer_activities.to_owned();
        let txids: Vec<bitcoin::Txid> = transfer_activities
            .clone()
            .into_iter()
            .map(|x| Txid::from_str(&x.tx_id.to_hex()).expect("invalid tx id"))
            .collect();
        prefetch_resolver_txs_status(txids, &mut resolver).await;

        for activity in transfer_activities {
            let ty = if activity.sender {
                TransferType::Sended
            } else {
                TransferType::Received
            };

            let txid = Txid::from_str(&activity.tx_id.to_hex()).expect("invalid tx id");
            let status = resolver
                .txs_status
                .get(&txid)
                .unwrap_or(&TxStatus::NotFound)
                .to_owned();

            let detail = RgbTransferDetail {
                consig_id: activity.consig_id,
                status,
                ty,
            };
            transfers.push(detail);
        }
    }

    Ok(RgbTransfersResponse { transfers })
}

pub async fn list_my_orders(sk: &str) -> Result<RgbOfferBidsResponse> {
    let rgb_offers = retrieve_offers(sk).await?;
    let rgb_bids = retrieve_bids(sk).await?;

    let mut offers = vec![];
    rgb_offers
        .offers
        .into_iter()
        .for_each(|(_, offs)| offers.extend(offs.into_iter().map(RgbOfferDetail::from)));

    let mut bids = vec![];
    rgb_bids
        .bids
        .into_iter()
        .for_each(|(_, bs)| bids.extend(bs.into_iter().map(RgbBidDetail::from)));

    Ok(RgbOfferBidsResponse { offers, bids })
}

pub async fn list_my_offers(sk: &str) -> Result<RgbOffersResponse> {
    let rgb_offers = retrieve_offers(sk).await?;

    let mut offers = vec![];
    rgb_offers
        .offers
        .into_iter()
        .for_each(|(_, offs)| offers.extend(offs.into_iter().map(RgbOfferDetail::from)));

    Ok(RgbOffersResponse { offers })
}

pub async fn list_my_bids(sk: &str) -> Result<RgbBidsResponse> {
    let rgb_bids = retrieve_bids(sk).await?;
    let mut bids = vec![];
    rgb_bids
        .bids
        .into_iter()
        .for_each(|(_, bs)| bids.extend(bs.into_iter().map(RgbBidDetail::from)));

    Ok(RgbBidsResponse { bids })
}

pub async fn list_public_offers(_sk: &str) -> Result<PublicRgbOffersResponse> {
    let rgb_public_offers = retrieve_public_offers().await?;

    let mut offers = vec![];
    let mut bids = BTreeMap::new();
    rgb_public_offers
        .rgb_offers
        .offers
        .into_iter()
        .for_each(|(_, offs)| offers.extend(offs.into_iter().map(PublicRgbOfferResponse::from)));

    rgb_public_offers
        .rgb_offers
        .bids
        .into_iter()
        .for_each(|(offer_id, bs)| {
            let bs = bs
                .values()
                .map(|x| PublicRgbBidResponse::from(x.to_owned()))
                .collect();
            bids.insert(offer_id, bs);
        });

    Ok(PublicRgbOffersResponse { offers, bids })
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ImportError {
    /// Some request data is missing. {0}
    Validation(String),
    /// I/O or connectivity error. {0}
    IO(RgbPersistenceError),
    /// Watcher is required for this operation.
    Watcher,
    /// Occurs an error in import step. {0}
    Import(ImportContractError),
    /// Occurs an error in export step. {0}
    Export(ExportContractError),
}

pub async fn import(sk: &str, request: ImportRequest) -> Result<ContractResponse, ImportError> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, mut rgb_account) = retrieve_stock_account(sk).await.map_err(ImportError::IO)?;

    let ImportRequest { data, import } = request;
    prefetch_resolver_import_rgb(&data, import.clone(), &mut resolver).await;

    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            prefetch_resolver_utxos(
                import.clone() as u32,
                &mut fetch_wallet,
                &mut resolver,
                Some(RGB_DEFAULT_FETCH_LIMIT),
            )
            .await;
            Some(fetch_wallet)
        }
        _ => None,
    };

    let contract =
        import_contract(&data, import, &mut stock, &mut resolver).map_err(ImportError::Import)?;
    let resp = export_contract(
        contract.contract_id(),
        &mut stock,
        &mut resolver,
        &mut wallet,
    )
    .map_err(ImportError::Export)?;

    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);
    };

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(ImportError::IO)?;

    Ok(resp)
}

// TODO: Extracte all watcher operations to watcher module
#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum WatcherError {
    /// Some request data is missing. {0}
    Validation(String),
    /// I/O or connectivity error. {0}
    IO(RgbPersistenceError),
    /// Watcher is required for this operation.
    NoWatcher,
    /// Occurs an error in parse descriptor step. {0}
    WrongDesc(String),
    /// Occurs an error in parse xpub step. {0}
    WrongXPub(String),
    /// Occurs an error in create watcher step. {0}
    Create(String),
    /// Occurs an error in migrate watcher step. {0}
    Legacy(String),
}

pub async fn create_watcher(
    sk: &str,
    request: WatcherRequest,
) -> Result<WatcherResponse, WatcherError> {
    let WatcherRequest { name, xpub, force } = request;
    let mut rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;

    if rgb_account.wallets.contains_key(&name) && force {
        rgb_account.wallets.remove(&name);
    }

    let mut migrate = false;
    if let Some(current_wallet) = rgb_account.wallets.get(&name) {
        let current_wallet = current_wallet.clone();
        let RgbDescr::Tapret(tapret_desc) = current_wallet.clone().descr;

        if xpub != tapret_desc.xpub.to_string() {
            rgb_account
                .wallets
                .insert("legacy".to_string(), current_wallet);
            rgb_account.wallets.remove(&name);
            migrate = true;
        }
    }

    if !rgb_account.wallets.contains_key(&name) {
        let xdesc = DescriptorPublicKey::from_str(&xpub)
            .map_err(|err| WatcherError::WrongDesc(err.to_string()))?;
        if let DescriptorPublicKey::XPub(xpub) = xdesc {
            let xpub = xpub.xkey;
            let xpub = ExtendedPubKey::from_str(&xpub.to_string())
                .map_err(|err| WatcherError::WrongXPub(err.to_string()))?;
            create_wallet(&name, xpub, &mut rgb_account.wallets)
                .map_err(|err| WatcherError::Create(err.to_string()))?;
        } else {
            return Err(WatcherError::WrongXPub("invalid xpub type".to_string()));
        }
    }

    store_account(sk, rgb_account)
        .await
        .map_err(WatcherError::IO)?;

    Ok(WatcherResponse { name, migrate })
}

pub async fn clear_watcher(sk: &str, name: &str) -> Result<WatcherResponse, WatcherError> {
    let mut rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;

    if rgb_account.wallets.contains_key(name) {
        rgb_account.wallets.remove(name);
    }

    store_account(sk, rgb_account)
        .await
        .map_err(WatcherError::IO)?;
    Ok(WatcherResponse {
        name: name.to_string(),
        migrate: false,
    })
}

pub async fn watcher_details(sk: &str, name: &str) -> Result<WatcherDetailResponse, WatcherError> {
    let (mut stock, mut rgb_account) =
        retrieve_stock_account(sk).await.map_err(WatcherError::IO)?;

    let mut wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => wallet.to_owned(),
        _ => return Err(WatcherError::NoWatcher),
    };

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let mut allocations = vec![];
    for contract_type in [AssetType::RGB20, AssetType::RGB21] {
        let iface_index = contract_type as u32;
        prefetch_resolver_utxos(
            iface_index,
            &mut wallet,
            &mut resolver,
            Some(RGB_DEFAULT_FETCH_LIMIT),
        )
        .await;
        prefetch_resolver_user_utxo_status(iface_index, &mut wallet, &mut resolver, false).await;
        let mut result = list_allocations(&mut wallet, &mut stock, iface_index, &mut resolver)
            .map_err(|op| WatcherError::Validation(op.to_string()))?;
        allocations.append(&mut result);
    }

    let resp = WatcherDetailResponse {
        contracts: allocations,
    };
    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_string(), wallet);

    store_stock_account(sk, stock, rgb_account)
        .await
        .map_err(WatcherError::IO)?;
    Ok(resp)
}

pub async fn watcher_address(
    sk: &str,
    name: &str,
    address: &str,
) -> Result<WatcherUtxoResponse, WatcherError> {
    let mut rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;

    let mut resp = WatcherUtxoResponse::default();
    if let Some(wallet) = rgb_account.wallets.get(name) {
        // Prefetch
        let mut resolver = ExplorerResolver {
            explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
            ..default!()
        };

        let asset_indexes: Vec<u32> = [0, 1, 9, 10, 20, 21].to_vec();
        let mut wallet = wallet.to_owned();

        prefetch_resolver_waddress(address, &mut wallet, &mut resolver, Some(20)).await;
        resp.utxos = register_address(address, asset_indexes, &mut wallet, &mut resolver, Some(20))
            .map_err(|op| WatcherError::Validation(op.to_string()))?
            .into_iter()
            .map(|utxo| utxo.outpoint.to_string())
            .collect();

        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);

        store_account(sk, rgb_account)
            .await
            .map_err(WatcherError::IO)?;
    };

    Ok(resp)
}

pub async fn watcher_utxo(
    sk: &str,
    name: &str,
    utxo: &str,
) -> Result<WatcherUtxoResponse, WatcherError> {
    let rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;

    let mut resp = WatcherUtxoResponse::default();
    if let Some(wallet) = rgb_account.wallets.get(name) {
        let network = NETWORK.read().await.to_string();
        let network =
            Network::from_str(&network).map_err(|op| WatcherError::Validation(op.to_string()))?;

        let mut resolver = ExplorerResolver {
            explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
            ..default!()
        };

        let network = AddressNetwork::from(network);
        let asset_indexes: Vec<u32> = [0, 1, 9, 10, 20, 21].to_vec();
        let mut wallet = wallet.to_owned();

        prefetch_resolver_wutxo(utxo, network, &mut wallet, &mut resolver, Some(20)).await;
        resp.utxos = register_utxo(
            utxo,
            network,
            asset_indexes,
            &mut wallet,
            &mut resolver,
            Some(RGB_DEFAULT_FETCH_LIMIT),
        )
        .map_err(|op| WatcherError::Validation(op.to_string()))?
        .into_iter()
        .map(|utxo| utxo.outpoint.to_string())
        .collect();
    };

    Ok(resp)
}

pub async fn watcher_next_address(
    sk: &str,
    name: &str,
    iface: &str,
) -> Result<NextAddressResponse, WatcherError> {
    let rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;

    let network = NETWORK.read().await.to_string();
    let network =
        Network::from_str(&network).map_err(|op| WatcherError::Validation(op.to_string()))?;
    let network = AddressNetwork::from(network);

    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => wallet.to_owned(),
        _ => return Err(WatcherError::NoWatcher),
    };

    let iface_index = match iface {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 10,
    };

    let next_address = next_address(iface_index, wallet, network)
        .map_err(|op| WatcherError::Validation(op.to_string()))?;

    let resp = NextAddressResponse {
        address: next_address.address.to_string(),
        network: network.to_string(),
    };
    Ok(resp)
}

pub async fn watcher_next_utxo(
    sk: &str,
    name: &str,
    iface: &str,
) -> Result<NextUtxoResponse, WatcherError> {
    let mut rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;
    let iface_index = match iface {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 10,
    };

    let mut wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => wallet.to_owned(),
        _ => return Err(WatcherError::NoWatcher),
    };

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    prefetch_resolver_utxos(
        iface_index,
        &mut wallet,
        &mut resolver,
        Some(RGB_DEFAULT_FETCH_LIMIT),
    )
    .await;
    prefetch_resolver_user_utxo_status(iface_index, &mut wallet, &mut resolver, true).await;
    sync_wallet(iface_index, &mut wallet, &mut resolver);

    let utxo = match next_utxo(iface_index, wallet.clone(), &mut resolver)
        .map_err(|op| WatcherError::Validation(op.to_string()))?
    {
        Some(next_utxo) => Some(UtxoResponse::with(
            next_utxo.outpoint,
            next_utxo.amount,
            next_utxo.status,
        )),
        _ => None,
    };

    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_string(), wallet);

    store_account(sk, rgb_account)
        .await
        .map_err(WatcherError::IO)?;

    Ok(NextUtxoResponse { utxo })
}

pub async fn watcher_unspent_utxos(
    sk: &str,
    name: &str,
    iface: &str,
) -> Result<NextUtxosResponse, WatcherError> {
    let mut rgb_account = retrieve_account(sk).await.map_err(WatcherError::IO)?;
    let mut wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => wallet.to_owned(),
        _ => return Err(WatcherError::NoWatcher),
    };

    let iface_index = match iface {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 10,
    };

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    sync_wallet(iface_index, &mut wallet, &mut resolver);
    prefetch_resolver_utxos(
        iface_index,
        &mut wallet,
        &mut resolver,
        Some(RGB_DEFAULT_FETCH_LIMIT),
    )
    .await;
    prefetch_resolver_user_utxo_status(iface_index, &mut wallet, &mut resolver, true).await;

    let utxos: HashSet<UtxoResponse> = next_utxos(iface_index, wallet.clone(), &mut resolver)
        .map_err(|op| WatcherError::Validation(op.to_string()))?
        .into_iter()
        .map(|x| UtxoResponse::with(x.outpoint, x.amount, x.status))
        .collect();

    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_string(), wallet);

    store_account(sk, rgb_account)
        .await
        .map_err(WatcherError::IO)?;

    Ok(NextUtxosResponse {
        utxos: utxos.into_iter().collect(),
    })
}

pub async fn clear_stock(sk: &str) {
    store_rgb_stock(sk, Stock::default())
        .await
        .expect("unable clear stock");
}

pub async fn get_consignment(consig_or_receipt_id: &str) -> Result<Option<String>> {
    let resp = get_rgb_consignment(consig_or_receipt_id).await?;
    Ok(resp)
}

pub async fn import_consignments(req: BTreeMap<String, String>) -> Result<bool> {
    post_consignments(req).await?;
    Ok(true)
}

pub async fn get_media_metadata(media_id: &str) -> Result<Option<MediaMetadata>> {
    let resp = get_rgb_media_metadata(media_id).await?;
    Ok(resp)
}

pub async fn import_uda_data(request: MediaRequest) -> Result<MediaResponse> {
    let mut resp = MediaResponse::default();

    if let Some(preview) = request.preview {
        let metadata = post_media_metadata(preview, MediaEncode::Base64).await?;
        resp.preview = Some(MediaView::new(metadata, MediaEncode::Base64));
    }

    if let Some(media) = request.media {
        let metadata = post_media_metadata(media, MediaEncode::Sha2).await?;
        resp.media = Some(MediaView::new(metadata, MediaEncode::Sha2));
    }

    let attachs = post_media_metadata_list(request.attachments, MediaEncode::Blake3).await?;
    for attach in attachs {
        resp.attachments
            .push(MediaView::new(attach, MediaEncode::Blake3))
    }

    Ok(resp)
}

pub async fn decode_invoice(invoice: String) -> Result<RgbInvoiceResponse> {
    let rgb_invoice = RgbInvoice::from_str(&invoice)?;

    let contract_id = rgb_invoice
        .contract
        .map(|x| x.to_string())
        .unwrap_or_default();

    let amount = match rgb_invoice.owned_state {
        TypedState::Amount(amount) => amount,
        TypedState::Data(_) => 1,
        _ => 0,
    };

    Ok(RgbInvoiceResponse {
        contract_id,
        amount,
    })
}

pub async fn inspect_contract(
    stock: &mut Stock,
    rgb_account: RgbAccountV1,
    contract_id: &str,
) -> Result<ContractResponse> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let contract_id = ContractId::from_str(contract_id)?;
    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                let contract_index = contract_type.clone() as u32;
                let iface_name = contract_type.to_string().to_uppercase().clone();

                let iface = stock
                    .iface_by_name(&tn!(iface_name.clone()))
                    .map_err(|_| TransferError::NoIface)?;

                if let Ok(contract_iface) = stock.contract_iface(contract_id, iface.iface_id()) {
                    sync_wallet(contract_index, &mut fetch_wallet, &mut resolver);
                    prefetch_resolver_allocations(contract_iface, &mut resolver, true).await;
                    prefetch_resolver_utxos(
                        contract_index,
                        &mut fetch_wallet,
                        &mut resolver,
                        Some(RGB_DEFAULT_FETCH_LIMIT),
                    )
                    .await;
                }
            }

            Some(fetch_wallet)
        }
        _ => None,
    };

    let contract = export_contract(contract_id, stock, &mut resolver, &mut wallet)?;
    Ok(contract)
}

pub async fn read_contract(sk: &str, contract_id: &str) -> Result<ContractResponse> {
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut stock, rgb_account) = retrieve_stock_account(sk).await?;

    let contract_id = ContractId::from_str(contract_id)?;
    let wallet = rgb_account.wallets.get(RGB_DEFAULT_NAME);
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                let contract_index = contract_type.clone() as u32;
                let iface_name = contract_type.to_string().to_uppercase().clone();

                let iface = stock
                    .iface_by_name(&tn!(iface_name.clone()))
                    .map_err(|_| TransferError::NoIface)?;

                if let Ok(contract_iface) = stock.contract_iface(contract_id, iface.iface_id()) {
                    sync_wallet(contract_index, &mut fetch_wallet, &mut resolver);
                    prefetch_resolver_allocations(contract_iface, &mut resolver, true).await;
                    prefetch_resolver_utxos(
                        contract_index,
                        &mut fetch_wallet,
                        &mut resolver,
                        Some(RGB_DEFAULT_FETCH_LIMIT),
                    )
                    .await;
                }
            }

            Some(fetch_wallet)
        }
        _ => None,
    };

    let contract = export_contract(contract_id, &mut stock, &mut resolver, &mut wallet)?;
    Ok(contract)
}
