use amplify::{
    confinement::{Confined, U32},
    hex::FromHex,
};
use bech32::{decode, FromBase32};
use rgb_schemata::{nia_schema, uda_schema};
use rgbstd::{
    containers::Contract,
    contract::Genesis,
    persistence::{Inventory, Stash, Stock},
    resolvers::ResolveHeight,
    validation::ResolveTx,
};
use strict_encoding::StrictDeserialize;

use crate::structs::AssetType;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
// TODO: Complete errors
pub enum ImportError {}

pub fn import_contract<R>(
    contract: &str,
    asset_type: AssetType,
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

    let confined: Confined<Vec<u8>, 0, { U32 }> =
        Confined::try_from_iter(serialized.iter().copied())
            .expect("invalid strict serialized data");

    let contract = match Genesis::from_strict_serialized::<{ U32 }>(confined.clone()) {
        Ok(genesis) => contract_from_genesis(genesis, asset_type),
        Err(_) => Contract::from_strict_serialized::<{ U32 }>(confined)
            .expect("invalid strict contract data"),
    };

    let contract_id = contract.contract_id();
    let contract = contract.validate(resolver).expect("invalid contract state");

    if !stock
        .contract_ids()
        .expect("contract_ids from stock")
        .contains(&contract_id)
    {
        stock
            .import_contract(contract.clone(), resolver)
            .expect("import contract failed");
    };

    Ok(contract)
}

pub fn contract_from_genesis(genesis: Genesis, asset_type: AssetType) -> Contract {
    let schema = match asset_type {
        AssetType::RGB20 => nia_schema(),
        AssetType::RGB21 => uda_schema(),
        _ => nia_schema(),
    };

    Contract::new(schema, genesis)
}
