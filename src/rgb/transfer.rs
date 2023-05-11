use std::str::FromStr;

use amplify::{confinement::Confined, hex::ToHex};
use bitcoin_30::psbt::Psbt as PSBT;
use bitcoin_hashes::hex::FromHex;
use bp::{Txid, Vout};
use psbt::{serialize::Serialize, Psbt};
use rgbstd::{
    containers::{Bindle, Transfer},
    contract::{ContractId, GraphSeal},
    interface::TypedState,
    persistence::{Inventory, Stash, Stock},
    resolvers::ResolveHeight,
    validation::{ResolveTx, Status},
};
use rgbwallet::{InventoryWallet, RgbInvoice, RgbTransport};
use seals::txout::CloseMethod;
use strict_encoding::{StrictDeserialize, TypeName};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum InvoiceError {
    InterfaceNotFound(String),
    ContractNotfound(ContractId),
    InvalidBlindSeal,
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum PaymentError {
    Invalid,
}

pub fn create_invoice(
    contract_id: &str,
    iface: &str,
    amount: u64,
    seal: &str,
    stock: &mut Stock,
) -> Result<RgbInvoice, InvoiceError> {
    let iface = match stock.iface_by_name(&TypeName::from_str(iface).expect("iface not found")) {
        Ok(iface) => iface,
        Err(_) => return Err(InvoiceError::InterfaceNotFound(iface.to_string())),
    };
    let contract_id = ContractId::from_str(contract_id).expect("invalid contract_id parse");
    if !stock
        .contract_ids()
        .expect("contract_ids from stock")
        .contains(&contract_id)
    {
        return Err(InvoiceError::ContractNotfound(contract_id));
    };

    let mut split = seal.split(':');
    let seal = match (split.next(), split.next(), split.next()) {
        (Some(_seal), Some(txid), Some(vout)) => {
            let txid = Txid::from_str(txid).expect("invalid txid");
            let vout = Vout::from_str(vout).expect("invalid vout");
            GraphSeal::tapret_first(txid, vout)
        }
        _ => return Err(InvoiceError::InvalidBlindSeal),
    };

    // Generate Invoice
    let invoice = RgbInvoice {
        transport: RgbTransport::UnspecifiedMeans,
        contract: Some(contract_id),
        iface: iface.name.clone(),
        operation: None,
        assignment: None,
        beneficiary: seal.to_concealed_seal().into(),
        owned_state: TypedState::Amount(amount),
        chain: None,
        unknown_query: none!(),
    };

    stock
        .store_seal_secret(seal)
        .expect("cannot be import seal information");

    Ok(invoice)
}

pub fn pay_invoice(
    invoice: String,
    psbt: String,
    stock: &mut Stock,
) -> Result<(Psbt, Bindle<Transfer>), PaymentError> {
    let invoice = RgbInvoice::from_str(&invoice).expect("invalid Invoice format");
    let psbt_file = Psbt::from_str(&psbt).expect("invalid PSBT format");

    let psbt =
        base64::decode(&base64::encode(&psbt_file.serialize())).expect("invalid PSBT format");
    let mut psbt_final = PSBT::deserialize(&psbt).expect("invalid PSBT format");
    let transfer = stock
        .pay(invoice, &mut psbt_final, CloseMethod::TapretFirst)
        .expect("pay_invoice failed");

    let psbt_file =
        Psbt::from_str(&PSBT::serialize(&psbt_final).to_hex()).expect("invalid PSBT format");
    Ok((psbt_file, transfer))
}

pub fn validate_transfer<R: ResolveTx>(
    transfer: String,
    resolver: &mut R,
) -> Result<(ContractId, Status), (ContractId, PaymentError)> {
    let serialized =
        Vec::<u8>::from_hex(&transfer).expect("invalid transfer hexadecimal format data");
    let confined = Confined::try_from_iter(serialized.iter().copied())
        .expect("invalid strict transfer format data");
    let confined_transfer = Transfer::from_strict_serialized::<{ usize::MAX }>(confined);

    let transfer = confined_transfer.expect("invalid strict transfer format data");
    let transfer_status = transfer
        .clone()
        .validate(resolver)
        .expect("transfer cannot be validated");

    match transfer_status.into_validation_status() {
        Some(status) => Ok((transfer.contract_id(), status)),
        _ => Err((transfer.contract_id(), PaymentError::Invalid)),
    }
}

pub fn accept_transfer<T>(
    transfer: String,
    force: bool,
    resolver: &mut T,
    stock: &mut Stock,
) -> Result<Bindle<Transfer>, (Bindle<Transfer>, PaymentError)>
where
    T: ResolveHeight + ResolveTx,
    T::Error: 'static,
{
    let serialized =
        Vec::<u8>::from_hex(&transfer).expect("invalid transfer hexadecimal format data");
    let confined = Confined::try_from_iter(serialized.iter().copied())
        .expect("invalid strict transfer format data");
    let confined_transfer = Transfer::from_strict_serialized::<{ usize::MAX }>(confined);

    let transfer = confined_transfer.expect("invalid strict transfer format data");
    let transfer_status = transfer
        .validate(resolver)
        .expect("transfer cannot be validated");

    let bindle = Bindle::new(transfer_status.clone());
    match stock.accept_transfer(transfer_status, resolver, force) {
        Ok(_) => Ok(bindle),
        _ => Err((bindle, PaymentError::Invalid)),
    }
}
