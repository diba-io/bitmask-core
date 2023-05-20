use amplify::{
    confinement::{Confined, U16},
    hex::FromHex,
};
use bech32::{decode, FromBase32};
use rgbstd::{
    containers::Contract,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::ResolveTx,
};
use strict_encoding::StrictDeserialize;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum ImportError {}

pub fn import_contract<R>(
    contract: &str,
    stock: &mut Stock,
    resolver: &mut R,
) -> Result<Contract, ImportError>
where
    R: ResolveHeight + ResolveTx,
    R::Error: 'static,
{
    let serialized = if contract.starts_with("rgb1") {
        let (_, serialized, _) =
            decode(contract).expect("invalid serialized contract (bech32m format)");
        Vec::<u8>::from_base32(&serialized).expect("invalid hexadecimal contract (bech32m format)")
    } else {
        Vec::<u8>::from_hex(contract).expect("invalid hexadecimal contract (baid58 format)")
    };

    let confined = Confined::try_from_iter(serialized.iter().copied())
        .expect("invalid strict serialized data");
    let contract = Contract::from_strict_serialized::<{ U16 }>(confined)
        .expect("invalid strict contract data");
    let contract = contract.validate(resolver).expect("invalid contract state");

    stock
        .import_contract(contract.clone(), resolver)
        .expect("import contract failed");

    Ok(contract)
}
