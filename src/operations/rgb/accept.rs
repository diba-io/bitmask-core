use std::{
    borrow::BorrowMut,
    ops::{Deref, DerefMut},
};

use amplify::{confinement::Confined, hex::FromHex};
use rgbstd::{
    containers::Transfer,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::Status,
};
use strict_encoding::StrictDeserialize;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum InventoryError {}

pub fn accept_transfer<R: ResolveHeight>(
    transfer: Transfer,
    mut stock: Stock,
    resolver: &mut R,
) -> Result<Status, InventoryError>
where
    R::Error: 'static,
{
    let status = stock.accept_transfer(transfer, resolver, false).expect("");
    Ok(status)
}
