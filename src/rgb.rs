use ::psbt::serialize::Serialize;
use amplify::hex::ToHex;
use anyhow::Result;
use rgbstd::{
    containers::BindleContent,
    persistence::{Inventory, Stash, Stock},
};
use strict_encoding::StrictSerialize;

pub mod accept;
pub mod constants;
pub mod issue;
pub mod psbt;
pub mod resolvers;
pub mod schemas;
pub mod structs;
pub mod transfer;
pub mod wallets;

use crate::{
    constants::BITCOIN_ELECTRUM_API,
    rgb::{
        issue::issue_contract as create_contract,
        psbt::{create_psbt as create_rgb_psbt, extract_commit},
        resolvers::ExplorerResolver,
        transfer::{
            accept_transfer as accept_rgb_transfer, create_invoice as create_rgb_invoice,
            pay_invoice,
        },
    },
    structs::{
        AcceptRequest, AcceptResponse, ContractDetail, ContractsResponse, InterfaceDetail,
        InterfacesResponse, InvoiceResult, IssueResponse, PsbtRequest, PsbtResponse,
        RgbTransferRequest, RgbTransferResponse, SchemaDetail, SchemasResponse,
    },
};

/// RGB Operations

pub async fn issue_contract(
    ticker: &str,
    name: &str,
    description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
    iface: &str,
) -> Result<IssueResponse> {
    // TODO: Get stock from Carbonado
    let mut stock = Stock::default();

    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };

    let contract = create_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        iface,
        seal,
        tx_resolver,
        &mut stock,
    )?;

    let contract_id = contract.contract_id().to_string();
    let genesis = contract.bindle().to_string();

    // TODO: Update Stock to Carbonado
    Ok(IssueResponse {
        contract_id,
        iface: iface.to_string(),
        genesis,
    })
}

pub async fn create_invoice(
    contract_id: &str,
    iface: &str,
    amount: u64,
    seal: &str,
) -> Result<InvoiceResult> {
    // TODO: Get stock from Carbonado
    let mut stock = Stock::default();
    let invoice = create_rgb_invoice(contract_id, iface, amount, seal, &mut stock)?;

    // TODO: Update Stock to Carbonado
    Ok(InvoiceResult {
        invoice: invoice.to_string(),
    })
}

pub async fn create_psbt(request: PsbtRequest) -> Result<PsbtResponse> {
    let PsbtRequest {
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
        input_tweak,
    } = request;

    // TODO: Pull from Carbonado (?)
    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };

    let psbt_file = create_rgb_psbt(
        descriptor_pub,
        asset_utxo,
        asset_utxo_terminal,
        change_index,
        bitcoin_changes,
        fee,
        input_tweak,
        &tx_resolver,
    )?;

    let psbt = PsbtResponse {
        psbt: Serialize::serialize(&psbt_file).to_hex(),
    };
    // TODO: Push to Carbonado (?)
    Ok(psbt)
}

pub async fn pay_asset(request: RgbTransferRequest) -> Result<RgbTransferResponse> {
    let RgbTransferRequest { rgb_invoice, psbt } = request;

    // TODO: Pull from Carbonado
    let mut stock = Stock::default();
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
    // TODO: Push to Carbonado
    Ok(consig)
}

pub async fn accept_transfer(request: AcceptRequest) -> Result<AcceptResponse> {
    let AcceptRequest { consignment } = request;

    // TODO: Pull from Carbonado
    let mut stock = Stock::default();

    let explorer_url = BITCOIN_ELECTRUM_API.read().await;
    let mut tx_resolver = ExplorerResolver {
        explorer_url: explorer_url.to_string(),
    };
    let resp = match accept_rgb_transfer(consignment, false, &mut tx_resolver, &mut stock) {
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

    // TODO: Push to Carbonado
    Ok(resp)
}

pub async fn list_contracts() -> Result<ContractsResponse> {
    let mut stock = Stock::default();

    let mut contracts = vec![];
    for schema_id in stock.schema_ids().expect("invalid schemas state") {
        let schema = stock.schema(schema_id).expect("invalid schemas state");
        for (iface_id, _) in schema.clone().iimpls.into_iter() {
            for contract_id in stock.contract_ids().expect("invalid contracts state") {
                if stock.contract_iface(contract_id, iface_id).is_ok() {
                    let face = stock.iface_by_id(iface_id).expect("invalid iface state");
                    let item = ContractDetail {
                        contract_id: contract_id.to_string(),
                        iface: face.name.to_string(),
                    };
                    contracts.push(item)
                }
            }
        }
    }

    Ok(ContractsResponse { contracts })
}

pub async fn list_interfaces() -> Result<InterfacesResponse> {
    let stock = Stock::default();

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

pub async fn list_schemas() -> Result<SchemasResponse> {
    let stock = Stock::default();

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
