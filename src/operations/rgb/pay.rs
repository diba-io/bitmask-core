use bitcoin_30::psbt::Psbt as PSBT;
use psbt::{serialize::Serialize, Psbt};
use rgbstd::{
    containers::{Bindle, Transfer},
    persistence::Stock,
    validation::{ResolveTx, Status},
};
use rgbwallet::{InventoryWallet, RgbInvoice};
use seals::txout::CloseMethod;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum PaymentError {
    Invalid,
}

pub fn pay_asset(
    invoice: RgbInvoice,
    psbt: Psbt,
    mut stock: Stock,
) -> Result<Bindle<Transfer>, PaymentError> {
    let psbt = base64::decode(&base64::encode(&psbt.serialize())).expect("invalid PSBT format");
    let mut psbt_final = PSBT::deserialize(&psbt).expect("invalid PSBT format");
    let transfer = stock
        .pay(invoice, &mut psbt_final, CloseMethod::TapretFirst)
        .expect("pay_asset failed");
    Ok(transfer)
}

pub fn validate_pay<R: ResolveTx>(
    transfer: Transfer,
    resolver: &mut R,
) -> Result<Status, PaymentError> {
    let transfer_status = transfer.validate(resolver).expect("validate_pay failed");
    match transfer_status.into_validation_status() {
        Some(status) => Ok(status),
        _ => Err(PaymentError::Invalid),
    }
}
