use amplify::{confinement::Confined, hex::FromHex};
use bitcoin_30::psbt::Psbt as PSBT;
use psbt::{serialize::Serialize, Psbt};
use rgbstd::{
    containers::{Bindle, Transfer},
    persistence::Stock,
    validation::ResolveTx,
};
use rgbwallet::{InventoryWallet, RgbInvoice};
use seals::txout::CloseMethod;
use strict_encoding::StrictDeserialize;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum PayAssetError {
    Fail,
}

pub fn pay_asset(
    invoice: RgbInvoice,
    psbt: Psbt,
    mut stock: Stock,
) -> Result<Bindle<Transfer>, PayAssetError> {
    let psbt = base64::decode(&base64::encode(&psbt.serialize())).expect("");
    let mut psbt_final = PSBT::deserialize(&psbt).expect("");
    let transfer = stock
        .pay(invoice, &mut psbt_final, CloseMethod::TapretFirst)
        .expect("");
    Ok(transfer)
}

pub fn valid_pay<R: ResolveTx>(transfer: String, resolver: &mut R) -> Result<Transfer, Transfer> {
    let bytes = Vec::<u8>::from_hex(&transfer).expect("");
    let confined: Confined<Vec<u8>, 0, { usize::MAX }> = Confined::try_from(bytes).expect("");
    let confined = Transfer::from_strict_serialized(confined).expect("");
    let status = confined.validate(resolver);
    status
}
