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
pub enum PaymentError {
    Fail,
    Invalid,
}

pub fn pay_asset(
    invoice: RgbInvoice,
    psbt: Psbt,
    mut stock: Stock,
) -> Result<Bindle<Transfer>, PaymentError> {
    let psbt = base64::decode(&base64::encode(&psbt.serialize())).expect("");
    let mut psbt_final = PSBT::deserialize(&psbt).expect("");
    let transfer = stock
        .pay(invoice, &mut psbt_final, CloseMethod::TapretFirst)
        .expect("");
    Ok(transfer)
}

pub fn validate_pay<R: ResolveTx>(
    transfer: Transfer,
    resolver: &mut R,
) -> Result<Status, PaymentError> {
    // let bytes = Vec::<u8>::from_hex(&transfer).expect("");
    // let confined: Confined<Vec<u8>, 0, { usize::MAX }> = Confined::try_from(bytes).expect("");
    // let confined = Transfer::from_strict_serialized(confined).expect("");
    let transfer_status = transfer.validate(resolver).expect("");
    match transfer_status.into_validation_status() {
        Some(status) => Ok(status),
        _ => Err(PaymentError::Invalid),
    }
}
