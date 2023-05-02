use amplify::{confinement::Confined, hex::FromHex};
use rgbstd::{
    containers::Contract,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
};
use strict_encoding::StrictDeserialize;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum ImportError {}

pub fn import_contract<R: ResolveHeight>(
    contract: &str,
    stock: &mut Stock,
    resolver: &mut R,
) -> Result<Contract, ImportError>
where
    R::Error: 'static,
{
    let serialized = Vec::<u8>::from_hex(contract).expect("invalid hex");
    let confined = Confined::try_from_iter(serialized.iter().copied()).expect("");
    let contract = Contract::from_strict_serialized::<{ usize::MAX }>(confined).expect("");

    let _ = stock.import_contract(contract.clone(), resolver).expect("");
    Ok(contract)
}
