use std::str::FromStr;

use ::psbt::serialize::Serialize;
use amplify::hex::ToHex;
use anyhow::{anyhow, Result};
use bech32::{encode, ToBase32};
use bitcoin_30::bip32::ExtendedPubKey;
use bitcoin_scripts::address::AddressNetwork;
use miniscript_crate::DescriptorPublicKey;
use rgbstd::{
    containers::BindleContent,
    interface::IfaceId,
    persistence::{Inventory, Stash},
};
use strict_encoding::{tn, StrictSerialize, TypeName};

pub mod accept;
pub mod carbonado;
pub mod constants;
pub mod import;
pub mod issue;
pub mod prefetch;
pub mod psbt;
pub mod resolvers;
pub mod schemas;
pub mod structs;
pub mod transfer;
pub mod wallet;

use crate::{
    constants::{
        storage_keys::{ASSETS_STOCK, ASSETS_WALLETS},
        BITCOIN_EXPLORER_API, NETWORK,
    },
    rgb::{
        carbonado::{retrieve_stock, store_stock},
        issue::issue_contract as create_contract,
        psbt::{create_psbt as create_rgb_psbt, extract_commit},
        resolvers::ExplorerResolver,
        transfer::{
            accept_transfer as accept_rgb_transfer, create_invoice as create_rgb_invoice,
            pay_invoice,
        },
    },
    structs::{
        AcceptRequest, AcceptResponse, ContractDetail, ContractsResponse, ImportRequest,
        ImportResponse, InterfaceDetail, InterfacesResponse, InvoiceRequest, InvoiceResponse,
        IssueRequest, IssueResponse, NextAddressReponse, NextUtxoReponse, PsbtRequest,
        PsbtResponse, RgbTransferRequest, RgbTransferResponse, SchemaDetail, SchemasResponse,
        WatcherDetailReponse, WatcherRequest, WatcherResponse,
    },
};

use self::{
    carbonado::{retrieve_wallets, store_wallets},
    import::import_contract,
    prefetch::{
        prefetch_resolve_commit_utxo, prefetch_resolve_psbt_tx, prefetch_resolve_spend,
        prefetch_resolve_watcher,
    },
    wallet::{create_wallet, list_allocations, next_address, next_utxo, sync_wallet},
};

/// RGB Operations
#[allow(clippy::too_many_arguments)]
pub async fn issue_contract(sk: &str, request: IssueRequest) -> Result<IssueResponse> {
    let IssueRequest {
        ticker,
        name,
        description,
        precision,
        supply,
        iface,
        seal,
    } = request;
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    // Resolvers Workaround
    let resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    let contract = create_contract(
        &ticker,
        &name,
        &description,
        precision,
        supply,
        &iface,
        &seal,
        resolver,
        &mut stock,
    )?;

    let contract_id = contract.contract_id().to_string();
    let contract = encode(
        "rgb",
        contract
            .bindle()
            .to_strict_serialized::<0xFFFFFF>()?
            .to_base32(),
        bech32::Variant::Bech32m,
    )?;

    store_stock(sk, ASSETS_STOCK, &stock).await?;

    Ok(IssueResponse {
        contract_id,
        iface: iface.to_string(),
        contract,
        issue_utxo: seal.replace("tapret1st:", ""),
    })
}

pub async fn create_invoice(sk: &str, request: InvoiceRequest) -> Result<InvoiceResponse> {
    let InvoiceRequest {
        contract_id,
        iface,
        seal,
        amount,
    } = request;
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    let invoice = create_rgb_invoice(&contract_id, &iface, amount, &seal, &mut stock)?;

    store_stock(sk, ASSETS_STOCK, &stock).await?;

    Ok(InvoiceResponse {
        invoice: invoice.to_string(),
    })
}

pub async fn create_psbt(sk: &str, request: PsbtRequest) -> Result<PsbtResponse> {
    let PsbtRequest {
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
        input_tweak,
    } = request;

    let stock: rgbstd::persistence::Stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    // Resolvers Workaround
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolve_psbt_tx(&asset_utxo, &mut resolver).await;

    let psbt_file = create_rgb_psbt(
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
        input_tweak,
        &resolver,
    )?;

    let psbt = PsbtResponse {
        psbt: Serialize::serialize(&psbt_file).to_hex(),
    };

    store_stock(sk, ASSETS_STOCK, &stock).await?;

    Ok(psbt)
}

pub async fn transfer_asset(sk: &str, request: RgbTransferRequest) -> Result<RgbTransferResponse> {
    let RgbTransferRequest { rgb_invoice, psbt } = request;

    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;
    let (psbt, transfer) = pay_invoice(rgb_invoice, psbt, &mut stock)?;

    let commit = extract_commit(psbt.clone())?;
    let psbt = psbt.to_string();
    let consig = RgbTransferResponse {
        consig_id: transfer.bindle_id().to_string(),
        consig: transfer
            .to_strict_serialized::<0xFFFFFF>()
            .expect("invalid transfer serialization")
            .to_hex(),
        psbt,
        commit,
    };

    store_stock(sk, ASSETS_STOCK, &stock).await?;

    Ok(consig)
}

pub async fn accept_transfer(sk: &str, request: AcceptRequest) -> Result<AcceptResponse> {
    let AcceptRequest { consignment, force } = request;

    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    // Resolvers Workaround
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolve_commit_utxo(&consignment, &mut resolver).await;

    let resp = match accept_rgb_transfer(consignment, force, &mut resolver, &mut stock) {
        Ok(transfer) => AcceptResponse {
            contract_id: transfer.contract_id().to_string(),
            transfer_id: transfer.transfer_id().to_string(),
            valid: true,
        },
        Err((transfer, _)) => AcceptResponse {
            contract_id: transfer.contract_id().to_string(),
            transfer_id: transfer.transfer_id().to_string(),
            valid: false,
        },
    };

    store_stock(sk, ASSETS_STOCK, &stock).await?;

    Ok(resp)
}

pub async fn list_contracts(sk: &str) -> Result<ContractsResponse> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    let mut contracts = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            for contract_id in stock.contract_ids().expect("invalid contracts state") {
                if stock.contract_iface(contract_id, iface_id).is_ok() {
                    let contract_iface = stock
                        .contract_iface(contract_id, iface_id)
                        .expect("invalid contracts state");
                    let ty: TypeName = tn!("Ticker");
                    let ticker = match contract_iface.global(ty) {
                        Ok(values) => values.to_vec()[0].to_string(),
                        _ => String::new(),
                    };

                    let ty: TypeName = tn!("Name");
                    let name = match contract_iface.global(ty) {
                        Ok(values) => values.to_vec()[0].to_string(),
                        _ => String::new(),
                    };

                    let ty: TypeName = tn!("Details");
                    let details = match contract_iface.global(ty) {
                        Ok(values) => values.to_vec()[0].to_string(),
                        _ => String::new(),
                    };
                    let face = stock.iface_by_id(iface_id).expect("invalid iface state");
                    let item = ContractDetail {
                        contract_id: contract_id.to_string(),
                        iface: face.name.to_string(),
                        ticker,
                        name,
                        details,
                    };
                    contracts.push(item)
                }
            }
        }
    }

    Ok(ContractsResponse { contracts })
}

pub async fn list_interfaces(sk: &str) -> Result<InterfacesResponse> {
    let stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    let mut interfaces = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, iimpl) in schema.clone().iimpls.into_iter() {
            let face = stock.iface_by_id(iface_id).expect("invalid iface state");

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
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        let mut ifaces = vec![];
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            let face = stock.iface_by_id(iface_id).expect("invalid iface state");
            ifaces.push(face.name.to_string());
        }
        schemas.push(SchemaDetail {
            schema: schema_id.to_string(),
            ifaces,
        })
    }

    Ok(SchemasResponse { schemas })
}

pub async fn import(sk: &str, request: ImportRequest) -> Result<ImportResponse> {
    let ImportRequest { data, import: _ } = request;
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;

    // Resolvers Workaround
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolve_commit_utxo(&data, &mut resolver).await;

    let contract = import_contract(&data, &mut stock, &mut resolver)?;
    let ifaces: Vec<String> = contract.ifaces.keys().map(|f| f.to_string()).collect();
    let iface_id = IfaceId::from_str(&ifaces[0]).expect("iface parse error");

    let contract_iface = stock
        .contract_iface(contract.contract_id(), iface_id.to_owned())
        .expect("invalid contracts state");
    let ty: TypeName = tn!("Ticker");
    let ticker = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let ty: TypeName = tn!("Name");
    let name = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let ty: TypeName = tn!("Details");
    let description = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let ty: TypeName = tn!("Precision");
    let precision = match contract_iface.global(ty) {
        Ok(values) => values.to_vec()[0].to_string(),
        _ => String::new(),
    };

    let mut supply = 0;
    let mut seal = String::new();
    for owned in &contract_iface.iface.assignments {
        if let Ok(allocations) = contract_iface.fungible(owned.name.clone()) {
            for allocation in allocations {
                supply = allocation.value;
                seal = allocation.owner.to_string();
            }
        }
    }

    store_stock(sk, ASSETS_STOCK, &stock).await?;

    let resp = ImportResponse {
        contract_id: contract.contract_id().to_string(),
        ticker,
        name,
        description,
        precision: precision,
        supply,
        seal,
        ifaces,
    };
    Ok(resp)
}

pub async fn create_watcher(sk: &str, request: WatcherRequest) -> Result<WatcherResponse> {
    let WatcherRequest { name, xpub } = request;
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    if rgb_account.wallets.contains_key(&name) {
        rgb_account.wallets.remove(&name);
    }
    let xdesc = DescriptorPublicKey::from_str(&xpub)?;
    if let DescriptorPublicKey::XPub(xpub) = xdesc {
        let xpub = xpub.xkey;
        let xpub = ExtendedPubKey::from_str(&xpub.to_string())?;
        create_wallet(&name, xpub, &mut rgb_account.wallets)?;
    }

    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;
    Ok(WatcherResponse { name })
}

pub async fn watcher_details(sk: &str, name: &str) -> Result<WatcherDetailReponse> {
    let mut stock = retrieve_stock(sk, ASSETS_STOCK).await?;
    let mut rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };
    let mut wallet = wallet?;

    // Resolvers Workaround
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolve_watcher(20, &mut wallet, &mut resolver).await;

    let allocations = list_allocations(&mut wallet, &mut stock, &mut resolver)?;

    let resp = WatcherDetailReponse {
        contracts: allocations,
    };

    rgb_account.wallets.insert(name.to_string(), wallet);
    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;
    Ok(resp)
}

pub async fn watcher_next_address(sk: &str, name: &str) -> Result<NextAddressReponse> {
    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let network = NETWORK.read().await.to_string();
    let network = AddressNetwork::from_str(&network)?;

    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };

    let iface_index = 20;
    let wallet = wallet?;
    let next_address = next_address(iface_index, wallet, network)?;

    let resp = NextAddressReponse {
        address: next_address.address.to_string(),
        network: network.to_string(),
    };
    Ok(resp)
}

pub async fn watcher_next_utxo(sk: &str, name: &str) -> Result<NextUtxoReponse> {
    let rgb_account = retrieve_wallets(sk, ASSETS_WALLETS).await?;

    let wallet = match rgb_account.wallets.get(name) {
        Some(wallet) => Ok(wallet.to_owned()),
        _ => Err(anyhow!("Wallet watcher not found")),
    };

    let iface_index = 20;
    let mut wallet = wallet?;

    // Resolvers Workaround
    let mut resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };
    prefetch_resolve_watcher(iface_index, &mut wallet, &mut resolver).await;
    prefetch_resolve_spend(iface_index, wallet.clone(), &mut resolver).await;

    sync_wallet(iface_index, &mut wallet, &mut resolver);
    let utxo = match next_utxo(iface_index, wallet, &mut resolver)? {
        Some(next_utxo) => next_utxo.outpoint.to_string(),
        _ => String::new(),
    };

    store_wallets(sk, ASSETS_WALLETS, &rgb_account).await?;

    Ok(NextUtxoReponse { utxo })
}
