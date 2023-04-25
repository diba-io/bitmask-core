use rgbstd::{
    containers::Transfer,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::Status,
};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum InventoryError {}

pub fn accept_transfer<R: ResolveHeight>(
    transfer: Transfer,
    force: bool,
    mut stock: Stock,
    resolver: &mut R,
) -> Result<Status, InventoryError>
where
    R::Error: 'static,
{
    let status = stock
        .accept_transfer(transfer, resolver, force)
        .expect("accept transfer failed");
    Ok(status)
}
