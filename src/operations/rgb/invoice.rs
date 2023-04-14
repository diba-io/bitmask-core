use rgbstd::{
    contract::ContractId,
    interface::{Iface, TypedState},
    persistence::{Stash, Stock},
};
use rgbwallet::{RgbInvoice, RgbTransport};
use seals::txout::{blind::BlindSeal, CloseMethod, TxPtr};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum InvoiceError {
    ContractNotfound(ContractId),
    InvalidBlindSeal,
}

pub fn create_invoice(
    contract_id: ContractId,
    iface: Iface,
    amount: u64,
    seal: BlindSeal<TxPtr>,
    stock: Stock,
) -> Result<RgbInvoice, InvoiceError> {
    if seal.method != CloseMethod::TapretFirst {
        return Err(InvoiceError::InvalidBlindSeal);
    }

    if !stock
        .contract_ids()
        .unwrap_or_else(|_| {
            panic!(
                "{}",
                InvoiceError::ContractNotfound(contract_id).to_string()
            )
        })
        .contains(&contract_id)
    {
        return Err(InvoiceError::ContractNotfound(contract_id));
    }

    // Generate Contract
    let invoice = RgbInvoice {
        transport: RgbTransport::UnspecifiedMeans,
        contract: contract_id,
        iface: iface.name,
        operation: None,
        assignment: None,
        beneficiary: seal.to_concealed_seal().into(),
        owned_state: TypedState::Amount(amount),
        chain: None,
        unknown_query: none!(),
    };

    Ok(invoice)
}
