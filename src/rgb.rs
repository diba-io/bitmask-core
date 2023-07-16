use std::str::FromStr;

use ::psbt::serialize::Serialize;
use amplify::hex::ToHex;
use anyhow::{anyhow, Result};
use bitcoin::Network;
use bitcoin_30::bip32::ExtendedPubKey;
use bitcoin_scripts::address::AddressNetwork;
use miniscript_crate::DescriptorPublicKey;
use rgbstd::{
    containers::BindleContent,
    contract::ContractId,
    persistence::{Stash, Stock},
};
use rgbwallet::psbt::DbcPsbtError;
use strict_encoding::StrictSerialize;
use thiserror::Error;

pub mod accept;
pub mod carbonado;
pub mod constants;
pub mod contract;
pub mod import;
pub mod issue;
pub mod prefetch;
pub mod psbt;
pub mod resolvers;
pub mod structs;
pub mod transfer;
pub mod wallet;

use crate::{
    constants::{
        get_network,
        storage_keys::{ASSETS_STOCK, ASSETS_WALLETS},
        BITCOIN_EXPLORER_API, NETWORK,
    },
    rgb::{
        carbonado::{force_store_stock, retrieve_stock, store_stock},
        constants::WALLET_UNAVALIABLE,
        issue::{issue_contract as create_contract, IssueContractError},
        psbt::{create_psbt as create_rgb_psbt, extract_commit},
        resolvers::ExplorerResolver,
        transfer::{
            accept_transfer as accept_rgb_transfer, create_invoice as create_rgb_invoice,
            pay_invoice,
        },
        wallet::list_allocations,
    },
    structs::{
        AcceptRequest, AcceptResponse, AssetType, ContractMetadata, ContractResponse,
        ContractsResponse, ImportRequest, InterfaceDetail, InterfacesResponse, InvoiceRequest,
        InvoiceResponse, IssueMetaRequest, IssueMetadata, IssueRequest, IssueResponse,
        NewCollectible, NextAddressResponse, NextUtxoResponse, NextUtxosResponse, PsbtFeeRequest,
        PsbtRequest, PsbtResponse, ReIssueRequest, ReIssueResponse, RgbTransferRequest,
        RgbTransferResponse, SchemaDetail, SchemasResponse, UDADetail, UtxoResponse,
        WatcherDetailResponse, WatcherRequest, WatcherResponse, WatcherUtxoResponse,
    },
};

use self::{
    carbonado::{retrieve_wallets, store_wallets},
    constants::{CARBONADO_UNAVALIABLE, RGB_DEFAULT_NAME, STOCK_UNAVALIABLE},
    contract::{export_contract, ExportContractError},
    import::{import_contract, ImportContractError},
    prefetch::{
        prefetch_resolver_import_rgb, prefetch_resolver_psbt, prefetch_resolver_rgb,
        prefetch_resolver_utxo_status, prefetch_resolver_utxos, prefetch_resolver_waddress,
        prefetch_resolver_wutxo,
    },
    psbt::{fee_estimate, save_commit, CreatePsbtError},
    transfer::{AcceptTransferError, NewInvoiceError, NewPaymentError},
    wallet::{
        create_wallet, next_address, next_utxo, next_utxos, register_address, register_utxo,
        sync_wallet,
    },
};

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum IssueError {
    // Some request data is missing. {0}
    Validation(String),
    /// Retrieve I/O or connectivity error. {1} in {0}
    Retrive(String, String),
    /// Write I/O or connectivity error. {1} in {0}
    Write(String, String),
    // Watcher is required for this operation.
    Watcher,
    // Occurs an error in issue step. {0}
    Issue(IssueContractError),
    // Occurs an error in export step. {0}
    Export(ExportContractError),
}

/// RGB Operations
pub async fn issue_contract(sk: &str, request: IssueRequest) -> Result<IssueResponse, IssueError> {
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

    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        IssueError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await.map_err(|_| {
        IssueError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            WALLET_UNAVALIABLE.to_string(),
        )
    })?;

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let network = get_network().await;
    let wallet = rgb_account.wallets.get("default");
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                prefetch_resolver_utxos(contract_type as u32, &mut fetch_wallet, &mut resolver)
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
        precision: _,
        balance: _,
        allocations: _,
        contract,
        genesis,
        meta,
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

    store_stock(sk, ASSETS_STOCK, &stock).await.map_err(|_| {
        IssueError::Write(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    store_wallets(sk, ASSETS_WALLETS, &rgb_account)
        .await
        .map_err(|_| {
            IssueError::Write(
                CARBONADO_UNAVALIABLE.to_string(),
                WALLET_UNAVALIABLE.to_string(),
            )
        })?;

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
        issue_method: "tapret1st".to_string(),
        issue_utxo: seal.replace("tapret1st:", ""),
        meta,
    })
}

pub async fn reissue_contract(
    sk: &str,
    request: ReIssueRequest,
) -> Result<ReIssueResponse, IssueError> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        IssueError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await.map_err(|_| {
        IssueError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            WALLET_UNAVALIABLE.to_string(),
        )
    })?;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let mut reissue_resp = vec![];
    for contract in request.contracts {
        let ContractResponse {
            ticker,
            name,
            description,
            supply,
            contract_id: _,
            iimpl_id: _,
            iface,
            precision,
            balance: _,
            allocations,
            contract: _,
            genesis: _,
            meta: contract_meta,
        } = contract;

        let seals: Vec<String> = allocations
            .into_iter()
            .map(|alloc| format!("tapret1st:{}", alloc.utxo))
            .collect();
        let seal = seals.first().unwrap().to_owned();

        // TODO: Move to rgb/issue sub-module
        let mut meta = None;
        if let Some(contract_meta) = contract_meta {
            meta = Some(match contract_meta.meta() {
                ContractMetadata::UDA(uda) => IssueMetaRequest(IssueMetadata::UDA(uda.media)),
                ContractMetadata::Collectible(colectibles) => {
                    let mut items = vec![];
                    for collectible_item in colectibles {
                        let UDADetail {
                            ticker,
                            name,
                            token_index: _,
                            description,
                            balance: _,
                            media,
                            allocations: _,
                        } = collectible_item;

                        let new_item = NewCollectible {
                            ticker,
                            name,
                            description,
                            media,
                        };

                        items.push(new_item);
                    }

                    IssueMetaRequest(IssueMetadata::Collectible(items))
                }
            })
        }

        let network = get_network().await;
        let wallet = rgb_account.wallets.get("default");
        let mut wallet = match wallet {
            Some(wallet) => {
                let mut fetch_wallet = wallet.to_owned();
                for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                    prefetch_resolver_utxos(contract_type as u32, &mut fetch_wallet, &mut resolver)
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
            precision: _,
            balance: _,
            allocations: _,
            contract,
            genesis,
            meta,
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
            issue_method: "tapret1st".to_string(),
            issue_utxo: seal.replace("tapret1st:", ""),
            meta,
        });
    }

    force_store_stock(sk, ASSETS_STOCK, &stock)
        .await
        .map_err(|_| {
            IssueError::Write(
                CARBONADO_UNAVALIABLE.to_string(),
                STOCK_UNAVALIABLE.to_string(),
            )
        })?;

    store_wallets(sk, ASSETS_WALLETS, &rgb_account)
        .await
        .map_err(|_| {
            IssueError::Write(
                CARBONADO_UNAVALIABLE.to_string(),
                WALLET_UNAVALIABLE.to_string(),
            )
        })?;

    Ok(ReIssueResponse {
        contracts: reissue_resp,
    })
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum InvoiceError {
    // Some request data is missing. {0}
    Validation(String),
    /// Retrieve I/O or connectivity error. {1} in {0}
    Retrive(String, String),
    /// Write I/O or connectivity error. {1} in {0}
    Write(String, String),
    // Occurs an error in invoice step. {0}
    Invoice(NewInvoiceError),
}

pub async fn create_invoice(
    sk: &str,
    request: InvoiceRequest,
) -> Result<InvoiceResponse, InvoiceError> {
    let InvoiceRequest {
        contract_id,
        iface,
        seal,
        amount,
        params,
    } = request;

    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        InvoiceError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let invoice = create_rgb_invoice(&contract_id, &iface, amount, &seal, params, &mut stock)
        .map_err(InvoiceError::Invoice)?;

    store_stock(sk, ASSETS_STOCK, &stock).await.map_err(|_| {
        InvoiceError::Write(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    Ok(InvoiceResponse {
        invoice: invoice.to_string(),
    })
}

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum TransferError {
    // Some request data is missing. {0}
    Validation(String),
    /// Retrieve I/O or connectivity error. {1} in {0}
    Retrive(String, String),
    /// Write I/O or connectivity error. {1} in {0}
    Write(String, String),
    // Watcher is required in this operation. Please, create watcher.
    NoWatcher,
    // Occurs an error in create step. {0}
    Create(CreatePsbtError),
    // Occurs an error in commitment step. {0}
    Commitment(DbcPsbtError),
    // Occurs an error in payment step. {0}
    Pay(NewPaymentError),
    // Occurs an error in accept step. {0}
    Accept(AcceptTransferError),
    // // Consignment cannot be encoded.
    WrongConsig(String),
}

pub async fn create_psbt(sk: &str, request: PsbtRequest) -> Result<PsbtResponse, TransferError> {
    let PsbtRequest {
        asset_inputs,
        asset_descriptor_change,
        asset_terminal_change,
        bitcoin_inputs,
        bitcoin_changes,
        fee,
    } = request;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        TransferError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await.map_err(|_| {
        TransferError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            WALLET_UNAVALIABLE.to_string(),
        )
    })?;

    if rgb_account.wallets.get("default").is_none() {
        return Err(TransferError::NoWatcher);
    }

    let mut all_inputs = asset_inputs.clone();
    all_inputs.extend(bitcoin_inputs.clone());
    for input_utxo in all_inputs.clone() {
        prefetch_resolver_psbt(&input_utxo.utxo, &mut resolver).await;
    }

    // Retrieve transaction fee
    let fee = match fee {
        PsbtFeeRequest::Value(fee) => fee,
        PsbtFeeRequest::FeeRate(fee_rate) => fee_estimate(
            asset_inputs,
            asset_descriptor_change,
            asset_terminal_change.clone(),
            bitcoin_inputs,
            bitcoin_changes.clone(),
            fee_rate,
            &mut resolver,
        ),
    };

    let wallet = rgb_account.wallets.get("default");
    let (psbt_file, change_terminal) = create_rgb_psbt(
        all_inputs,
        bitcoin_changes,
        asset_terminal_change,
        fee,
        wallet.cloned(),
        &resolver,
    )
    .map_err(TransferError::Create)?;

    store_stock(sk, ASSETS_STOCK, &stock).await.map_err(|_| {
        TransferError::Write(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let psbt = PsbtResponse {
        psbt: Serialize::serialize(&psbt_file).to_hex(),
        terminal: change_terminal,
    };

    Ok(psbt)
}

pub async fn transfer_asset(
    sk: &str,
    request: RgbTransferRequest,
) -> Result<RgbTransferResponse, TransferError> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        TransferError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await.map_err(|_| {
        TransferError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            WALLET_UNAVALIABLE.to_string(),
        )
    })?;

    if rgb_account.wallets.get("default").is_none() {
        return Err(TransferError::NoWatcher);
    }

    let RgbTransferRequest {
        rgb_invoice,
        psbt,
        terminal,
    } = request;
    let (psbt, transfer) =
        pay_invoice(rgb_invoice, psbt, &mut stock).map_err(TransferError::Pay)?;

    let commit = extract_commit(psbt.clone()).map_err(TransferError::Commitment)?;
    let wallet = rgb_account.wallets.get("default");
    if let Some(wallet) = wallet {
        let mut wallet = wallet.to_owned();
        save_commit(&terminal, commit.clone(), &mut wallet);

        rgb_account
            .wallets
            .insert("default".to_string(), wallet.clone());

        store_wallets(sk, ASSETS_WALLETS, &rgb_account)
            .await
            .map_err(|_| {
                TransferError::Write(
                    CARBONADO_UNAVALIABLE.to_string(),
                    WALLET_UNAVALIABLE.to_string(),
                )
            })?;
    };

    let consig = transfer
        .to_strict_serialized::<{ usize::MAX }>()
        .map_err(|err| TransferError::WrongConsig(err.to_string()))?
        .to_hex();
    let consig_id = transfer.bindle_id().to_string();
    let commit = commit.to_hex();
    let psbt = psbt.to_string();

    let resp = RgbTransferResponse {
        consig_id,
        consig,
        psbt,
        commit,
    };

    store_stock(sk, ASSETS_STOCK, &stock).await.map_err(|_| {
        TransferError::Write(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    Ok(resp)
}

pub async fn accept_transfer(
    sk: &str,
    request: AcceptRequest,
) -> Result<AcceptResponse, TransferError> {
    let AcceptRequest { consignment, force } = request;
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        TransferError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;
    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolver_rgb(&consignment, &mut resolver, None).await;

    let transfer = accept_rgb_transfer(consignment, force, &mut resolver, &mut stock)
        .map_err(TransferError::Accept)?;

    let resp = AcceptResponse {
        contract_id: transfer.contract_id().to_string(),
        transfer_id: transfer.transfer_id().to_string(),
        valid: true,
    };

    store_stock(sk, ASSETS_STOCK, &stock).await.map_err(|_| {
        TransferError::Write(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    Ok(resp)
}

pub async fn get_contract(sk: &str, contract_id: &str) -> Result<ContractResponse> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let wallet = rgb_account.wallets.get("default");
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                prefetch_resolver_utxos(contract_type as u32, &mut fetch_wallet, &mut resolver)
                    .await;
            }

            Some(fetch_wallet)
        }
        _ => None,
    };

    let contract_id = ContractId::from_str(contract_id)?;
    let contract = export_contract(contract_id, &mut stock, &mut resolver, &mut wallet)?;

    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);
        store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;
    };

    Ok(contract)
}

pub async fn list_contracts(sk: &str) -> Result<ContractsResponse> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let wallet = rgb_account.wallets.get("default");
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            for contract_type in [AssetType::RGB20, AssetType::RGB21] {
                prefetch_resolver_utxos(contract_type as u32, &mut fetch_wallet, &mut resolver)
                    .await;
            }
            Some(fetch_wallet)
        }
        _ => None,
    };

    let mut contracts = vec![];

    for contract_id in stock.contract_ids()? {
        let resp = export_contract(contract_id, &mut stock, &mut resolver, &mut wallet)?;
        contracts.push(resp);
    }

    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);
        store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;
    };

    Ok(ContractsResponse { contracts })
}

pub async fn list_interfaces(sk: &str) -> Result<InterfacesResponse> {
    let stock = retrieve_stock(sk, ASSETS_STOCK).await?;

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
    let stock = retrieve_stock(sk, ASSETS_STOCK).await?;

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

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ImportError {
    // Some request data is missing. {0}
    Validation(String),
    /// Retrieve I/O or connectivity error. {1} in {0}
    Retrive(String, String),
    /// Write I/O or connectivity error. {1} in {0}
    Write(String, String),
    // Watcher is required for this operation.
    Watcher,
    // Occurs an error in import step. {0}
    Import(ImportContractError),
    // Occurs an error in export step. {0}
    Export(ExportContractError),
}

pub async fn import(sk: &str, request: ImportRequest) -> Result<ContractResponse, ImportError> {
    let ImportRequest { data, import } = request;
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await.map_err(|_| {
        ImportError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;

    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await.map_err(|_| {
        ImportError::Retrive(
            CARBONADO_UNAVALIABLE.to_string(),
            WALLET_UNAVALIABLE.to_string(),
        )
    })?;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolver_import_rgb(&data, import.clone(), &mut resolver).await;

    let wallet = rgb_account.wallets.get("default");
    let mut wallet = match wallet {
        Some(wallet) => {
            let mut fetch_wallet = wallet.to_owned();
            prefetch_resolver_utxos(import.clone() as u32, &mut fetch_wallet, &mut resolver).await;
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

    store_stock(sk, ASSETS_STOCK, &stock).await.map_err(|_| {
        ImportError::Write(
            CARBONADO_UNAVALIABLE.to_string(),
            STOCK_UNAVALIABLE.to_string(),
        )
    })?;
    if let Some(wallet) = wallet {
        rgb_account
            .wallets
            .insert(RGB_DEFAULT_NAME.to_string(), wallet);

        store_wallets(sk, ASSETS_WALLETS, &rgb_account)
            .await
            .map_err(|_| {
                ImportError::Write(
                    CARBONADO_UNAVALIABLE.to_string(),
                    WALLET_UNAVALIABLE.to_string(),
                )
            })?;
    };

    Ok(resp)
}

pub async fn create_watcher(
    sk: &str,
    request: WatcherRequest,
) -> Result<WatcherResponse, IssueError> {
    let WatcherRequest { name, xpub, force } = request;
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await.expect("");

    if rgb_account.wallets.contains_key(&name) && force {
        rgb_account.wallets.remove(&name);
    }

    if !rgb_account.wallets.contains_key(&name) {
        let xdesc = DescriptorPublicKey::from_str(&xpub).expect("");
        if let DescriptorPublicKey::XPub(xpub) = xdesc {
            let xpub = xpub.xkey;
            let xpub = ExtendedPubKey::from_str(&xpub.to_string()).expect("");
            create_wallet(&name, xpub, &mut rgb_account.wallets).expect("");
        }
    }

    store_wallets(sk, ASSETS_WALLETS, &rgb_account)
        .await
        .expect("");
    Ok(WatcherResponse { name })
}

pub async fn clear_watcher(sk: &str, name: &str) -> Result<WatcherResponse> {
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    if rgb_account.wallets.contains_key(name) {
        rgb_account.wallets.remove(name);
    }

    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;
    Ok(WatcherResponse {
        name: name.to_string(),
    })
}

pub async fn watcher_details(sk: &str, name: &str) -> Result<WatcherDetailResponse> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };
    let mut wallet = wallet?;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let mut allocations = vec![];
    for contract_type in [AssetType::RGB20, AssetType::RGB21] {
        let iface_index = contract_type as u32;
        prefetch_resolver_utxos(iface_index, &mut wallet, &mut resolver).await;
        prefetch_resolver_utxo_status(iface_index, &mut wallet, &mut resolver).await;
        let result = list_allocations(&mut wallet, &mut stock, iface_index, &mut resolver)?;
        allocations.extend(result);
    }

    let resp = WatcherDetailResponse {
        contracts: allocations,
    };
    store_stock(sk, ASSETS_STOCK, &stock).await?;

    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_string(), wallet);
    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;
    Ok(resp)
}

pub async fn watcher_address(sk: &str, name: &str, address: &str) -> Result<WatcherUtxoResponse> {
    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let mut resp = WatcherUtxoResponse::default();
    if let Some(wallet) = rgb_account.wallets.get(name) {
        // Prefetch
        let mut resolver = ExplorerResolver {
            explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
            ..Default::default()
        };

        let asset_indexes: Vec<u32> = [0, 1, 9, 20, 21].to_vec();
        let mut wallet = wallet.to_owned();

        prefetch_resolver_waddress(address, &mut wallet, &mut resolver, Some(20)).await;
        resp.utxos =
            register_address(address, asset_indexes, &mut wallet, &mut resolver, Some(20))?
                .into_iter()
                .map(|utxo| utxo.outpoint.to_string())
                .collect();
    };

    Ok(resp)
}

pub async fn watcher_utxo(sk: &str, name: &str, utxo: &str) -> Result<WatcherUtxoResponse> {
    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let mut resp = WatcherUtxoResponse::default();
    if let Some(wallet) = rgb_account.wallets.get(name) {
        // Prefetch
        let mut resolver = ExplorerResolver {
            explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
            ..Default::default()
        };
        let network = NETWORK.read().await.to_string();
        let network = Network::from_str(&network)?;
        let network = AddressNetwork::from(network);

        let asset_indexes: Vec<u32> = [0, 1, 9, 20, 21].to_vec();
        let mut wallet = wallet.to_owned();

        prefetch_resolver_wutxo(utxo, network, &mut wallet, &mut resolver, Some(20)).await;
        resp.utxos = register_utxo(
            utxo,
            network,
            asset_indexes,
            &mut wallet,
            &mut resolver,
            Some(20),
        )?
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
) -> Result<NextAddressResponse> {
    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let network = NETWORK.read().await.to_string();
    let network = Network::from_str(&network)?;
    let network = AddressNetwork::from(network);

    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };

    let iface_index = match iface {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 10,
    };

    let wallet = wallet?;
    let next_address = next_address(iface_index, wallet, network)?;

    let resp = NextAddressResponse {
        address: next_address.address.to_string(),
        network: network.to_string(),
    };
    Ok(resp)
}

pub async fn watcher_next_utxo(sk: &str, name: &str, iface: &str) -> Result<NextUtxoResponse> {
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;
    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };

    let iface_index = match iface {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 10,
    };

    let mut wallet = wallet?;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    prefetch_resolver_utxos(iface_index, &mut wallet, &mut resolver).await;
    prefetch_resolver_utxo_status(iface_index, &mut wallet, &mut resolver).await;

    sync_wallet(iface_index, &mut wallet, &mut resolver);
    let utxo = match next_utxo(iface_index, wallet.clone(), &mut resolver)? {
        Some(next_utxo) => Some(UtxoResponse {
            outpoint: next_utxo.outpoint.to_string(),
            amount: next_utxo.amount,
        }),
        _ => None,
    };

    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_string(), wallet);
    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;

    Ok(NextUtxoResponse { utxo })
}

pub async fn watcher_unspent_utxos(sk: &str, name: &str, iface: &str) -> Result<NextUtxosResponse> {
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;
    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };

    let iface_index = match iface {
        "RGB20" => 20,
        "RGB21" => 21,
        _ => 9,
    };

    let mut wallet = wallet?;

    // Prefetch
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    prefetch_resolver_utxos(iface_index, &mut wallet, &mut resolver).await;
    prefetch_resolver_utxo_status(iface_index, &mut wallet, &mut resolver).await;

    sync_wallet(iface_index, &mut wallet, &mut resolver);
    let utxos = next_utxos(iface_index, wallet.clone(), &mut resolver)?
        .into_iter()
        .map(|x| UtxoResponse {
            outpoint: x.outpoint.to_string(),
            amount: x.amount,
        })
        .collect();

    rgb_account
        .wallets
        .insert(RGB_DEFAULT_NAME.to_string(), wallet);
    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;

    Ok(NextUtxosResponse { utxos })
}

pub async fn clear_stock(sk: &str) {
    store_stock(sk, ASSETS_STOCK, &Stock::default())
        .await
        .expect("unable store stock");
}
