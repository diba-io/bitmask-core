use bitcoin_30::psbt::Psbt as PSBT;
use psbt::{serialize::Serialize, Psbt};
use rgbstd::{
    containers::{Bindle, Transfer},
    persistence::Stock,
};
use rgbwallet::{InventoryWallet, RgbInvoice};
use seals::txout::CloseMethod;

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
