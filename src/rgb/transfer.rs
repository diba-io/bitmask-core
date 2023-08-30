use std::{collections::HashMap, str::FromStr};

use amplify::{
    confinement::{Confined, U32},
    hex::ToHex,
};
use bitcoin_30::psbt::Psbt as PSBT;
use bitcoin_hashes::hex::FromHex;
use bp::{seals::txout::CloseMethod, Chain, Txid};
use indexmap::IndexMap;
use psbt::{serialize::Serialize, Psbt};
use rgbstd::{
    containers::{Bindle, Transfer},
    contract::{ContractId, GraphSeal},
    interface::TypedState,
    persistence::{Inventory, Stash, Stock},
    resolvers::ResolveHeight,
    validation::{AnchoredBundle, ConsignmentApi, ResolveTx, Status},
};
use rgbwallet::{InventoryWallet, InvoiceParseError, RgbInvoice, RgbTransport};
use seals::txout::ExplicitSeal;
use strict_encoding::{StrictDeserialize, TypeName};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum NewInvoiceError {
    /// '{0}' is an invalid iface name
    WrongIface(String),
    /// '{0}' is an invalid contract id
    WrongContract(String),
    /// '{0}' is an invalid seal definition
    WrongSeal(String),
    /// Network cannot be decoded. {0}
    WrongNetwork(String),
    /// {0} is unspecified or wrong contract id
    NoContract(String),
    /// There are no contracts defined in Stash
    EmptyContracts,
    /// Error saving secret seal: {0}
    StoreSeal(String),
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum NewPaymentError {
    Invalid,
    /// '{0}' is an invalid invoice format
    WrongInvoice(InvoiceParseError),
    /// PSBT data have an invalid hexadecimal format.
    WrongHex,
    /// PSBT file cannot be decoded. {0}
    WrongPSBT(String),
    /// Consignmnet has not been completed. {0}
    NoPay(String),
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum AcceptTransferError {
    /// Consignment data have an invalid hexadecimal format.
    WrongHex,
    /// ContractID cannot be decoded. {0}
    WrongContract(String),
    /// Consignment cannot be decoded. {0}
    WrongConsig(String),
    /// Network cannot be decoded. {0}
    WrongNetwork(String),
    /// The Consignment is invalid. Details: {0:?}
    InvalidConsig(Vec<String>),
    /// The Consignment is invalid (Unexpected behavior on validation).
    Inconclusive,
}

pub fn create_invoice(
    contract_id: &str,
    iface: &str,
    amount: u64,
    seal: &str,
    network: &str,
    params: HashMap<String, String>,
    stock: &mut Stock,
) -> Result<RgbInvoice, NewInvoiceError> {
    let ty =
        TypeName::from_str(iface).map_err(|_| NewInvoiceError::WrongIface(iface.to_string()))?;
    let iface = stock
        .iface_by_name(&ty)
        .map_err(|_| NewInvoiceError::WrongIface(iface.to_string()))?;

    let contract_id = ContractId::from_str(contract_id)
        .map_err(|_| NewInvoiceError::NoContract(contract_id.to_string()))?;

    // Temporary removal
    // if !stock
    //     .contract_ids()
    //     .map_err(|_| NewInvoiceError::EmptyContracts)?
    //     .contains(&contract_id)
    // {
    //     return Err(NewInvoiceError::NoContract(contract_id.to_string()));
    // };

    let chain =
        Chain::from_str(network).map_err(|op| NewInvoiceError::WrongNetwork(op.to_string()))?;

    let seal = ExplicitSeal::<Txid>::from_str(seal)
        .map_err(|_| NewInvoiceError::WrongIface(seal.to_string()))?;
    let seal = GraphSeal::new(seal.method, seal.txid, seal.vout);
    // Query Params
    let mut query = IndexMap::default();
    for (k, v) in params {
        query.insert(k, v);
    }

    // Generate Invoice
    let invoice = RgbInvoice {
        transports: vec![RgbTransport::UnspecifiedMeans],
        contract: Some(contract_id),
        iface: Some(iface.name.clone()),
        operation: None,
        assignment: None,
        beneficiary: seal.to_concealed_seal().into(),
        owned_state: TypedState::Amount(amount),
        chain: Some(chain),
        unknown_query: query,
        expiry: None,
    };

    stock
        .store_seal_secret(seal)
        .map_err(|op| NewInvoiceError::StoreSeal(op.to_string()))?;

    Ok(invoice)
}

pub fn pay_invoice(
    invoice: String,
    psbt: String,
    stock: &mut Stock,
) -> Result<(Psbt, Bindle<Transfer>), NewPaymentError> {
    let invoice = RgbInvoice::from_str(&invoice).map_err(NewPaymentError::WrongInvoice)?;
    let psbt_file = Psbt::from_str(&psbt).map_err(|_| NewPaymentError::WrongHex)?;

    let psbt = base64::decode(&base64::encode(&psbt_file.serialize()))
        .map_err(|err| NewPaymentError::WrongPSBT(err.to_string()))?;

    let mut psbt_final =
        PSBT::deserialize(&psbt).map_err(|err| NewPaymentError::WrongPSBT(err.to_string()))?;

    let transfer = stock
        .pay(invoice, &mut psbt_final, CloseMethod::TapretFirst)
        .map_err(|err| NewPaymentError::NoPay(err.to_string()))?;

    let psbt_file = Psbt::from_str(&PSBT::serialize(&psbt_final).to_hex())
        .map_err(|err| NewPaymentError::WrongPSBT(err.to_string()))?;
    Ok((psbt_file, transfer))
}

pub fn validate_transfer<R: ResolveTx>(
    transfer: String,
    resolver: &mut R,
) -> Result<(ContractId, Status), AcceptTransferError> {
    let serialized = Vec::<u8>::from_hex(&transfer).map_err(|_| AcceptTransferError::WrongHex)?;
    let confined = Confined::try_from_iter(serialized.iter().copied())
        .map_err(|err| AcceptTransferError::WrongConsig(err.to_string()))?;
    let transfer = Transfer::from_strict_serialized::<{ usize::MAX }>(confined)
        .map_err(|err| AcceptTransferError::WrongConsig(err.to_string()))?;

    let consig = transfer.clone().validate(resolver).map_err(|err| {
        if let Some(status) = err.into_validation_status() {
            let mut messages = vec![];
            messages.append(&mut status.warnings.into_iter().map(|x| x.to_string()).collect());
            messages.append(&mut status.failures.into_iter().map(|x| x.to_string()).collect());
            AcceptTransferError::InvalidConsig(messages)
        } else {
            AcceptTransferError::Inconclusive
        }
    })?;

    let status = consig.into_validation_status();
    Ok((transfer.contract_id(), status.unwrap_or_default()))
}

pub fn accept_transfer<T>(
    transfer: String,
    force: bool,
    resolver: &mut T,
    stock: &mut Stock,
) -> Result<Bindle<Transfer>, AcceptTransferError>
where
    T: ResolveHeight + ResolveTx,
    T::Error: 'static,
{
    let serialized = Vec::<u8>::from_hex(&transfer).map_err(|_| AcceptTransferError::WrongHex)?;
    let confined = Confined::try_from_iter(serialized.iter().copied())
        .map_err(|err| AcceptTransferError::WrongConsig(err.to_string()))?;
    let transfer = Transfer::from_strict_serialized::<{ U32 }>(confined)
        .map_err(|err| AcceptTransferError::WrongConsig(err.to_string()))?;

    let consig = transfer.validate(resolver).map_err(|err| {
        if let Some(status) = err.into_validation_status() {
            let mut messages = vec![];
            messages.append(&mut status.warnings.into_iter().map(|x| x.to_string()).collect());
            messages.append(&mut status.failures.into_iter().map(|x| x.to_string()).collect());
            AcceptTransferError::InvalidConsig(messages)
        } else {
            AcceptTransferError::Inconclusive
        }
    })?;

    let bindle = Bindle::new(consig.clone());
    match stock.accept_transfer(consig, resolver, force) {
        Ok(_) => Ok(bindle),
        Err(err) => Err(AcceptTransferError::InvalidConsig(vec![err.to_string()])),
    }
}

pub fn extract_transfer(transfer: String) -> Result<(Txid, Bindle<Transfer>), AcceptTransferError> {
    let serialized = Vec::<u8>::from_hex(&transfer).map_err(|_| AcceptTransferError::WrongHex)?;
    let confined = Confined::try_from_iter(serialized.iter().copied())
        .map_err(|err| AcceptTransferError::WrongConsig(err.to_string()))?;
    let transfer = Transfer::from_strict_serialized::<{ U32 }>(confined)
        .map_err(|err| AcceptTransferError::WrongConsig(err.to_string()))?;

    for (bundle_id, _) in transfer.terminals() {
        if transfer.known_transitions_by_bundle_id(bundle_id).is_none() {
            return Err(AcceptTransferError::Inconclusive);
        };
        if let Some(AnchoredBundle { anchor, bundle: _ }) = transfer.anchored_bundle(bundle_id) {
            return Ok((anchor.txid, Bindle::new(transfer)));
        }
    }

    Err(AcceptTransferError::Inconclusive)
}
