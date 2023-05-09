use std::str::FromStr;

use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::ResolveTx;

use amplify::confinement::U16;
use bp::seals::txout::ExplicitSeal;
use rgbstd::containers::Contract;
use rgbstd::contract::GenesisSeal;
use rgbstd::interface::{BuilderError, ContractBuilder, Iface};
use rgbstd::persistence::{Inventory, Stash, Stock};
use rgbstd::Txid;
use strict_types::encoding::TypeName;
use strict_types::{svstr, svstruct, StrictVal};

use super::schemas::{default_fungible_iimpl, default_fungible_schema};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum IssueError {
    Forge(BuilderError),
    ContractNotfound(String),
    ImportContract(String),
    ContractInvalid(String),
}

#[allow(clippy::too_many_arguments)]
pub fn issue_contract<T>(
    ticker: &str,
    name: &str,
    description: &str,
    precision: u8,
    supply: u64,
    iface: &str,
    seal: &str,
    mut resolver: T,
    stock: &mut Stock,
) -> Result<Contract, IssueError>
where
    T: ResolveHeight + ResolveTx,
    T::Error: 'static,
{
    let iface_name = match TypeName::from_str(iface) {
        Ok(name) => name,
        _ => return Err(IssueError::Forge(BuilderError::InterfaceMismatch)),
    };

    let binding = stock.to_owned();
    let iface = match binding.iface_by_name(&iface_name) {
        Ok(name) => name,
        _ => return Err(IssueError::Forge(BuilderError::InterfaceMismatch)),
    };

    let contract_issued = match iface.name.as_str() {
        "RGB20" => issue_fungible_asset(ticker, name, description, precision, supply, iface, seal),
        _ => return Err(IssueError::ContractNotfound(iface.name.to_string())),
    };

    let resp = match contract_issued {
        Ok(resp) => resp,
        Err(err) => return Err(IssueError::Forge(err)),
    };

    let resp = match resp.clone().validate(&mut resolver) {
        Ok(resp) => resp,
        Err(_) => return Err(IssueError::ContractInvalid(resp.contract_id().to_string())),
    };

    unsafe {
        stock
            .import_contract_force(resp.clone(), &mut resolver)
            .or(Err(IssueError::ImportContract(
                resp.contract_id().to_string(),
            )))
    }?;

    Ok(resp)
}

/// RGB20 interface
fn issue_fungible_asset(
    ticker: &str,
    name: &str,
    description: &str,
    precision: u8,
    supply: u64,
    iface: &Iface,
    seal: &str,
) -> Result<Contract, BuilderError> {
    let iimpl = default_fungible_iimpl();
    let schema = default_fungible_schema();
    let types = &schema.type_system;

    let global_state_map = bmap! {
        "Nominal" => svstruct!(name => name, ticker => ticker, details => StrictVal::none(), precision => precision),
        "ContractText" => svstr!(description)
    };

    let mut builder = ContractBuilder::with(iface.to_owned(), schema.clone(), iimpl.clone())
        .expect("schema fails to implement RGB20 interface");

    // Global State
    for global in iimpl.global_state.iter() {
        let type_id = global.id;
        let type_name = global.name.as_str();
        let sem_id = schema
            .global_types
            .get(&type_id)
            .expect("invalid schema implementation")
            .sem_id;

        let data = global_state_map
            .get(type_name)
            .expect("global state data not found")
            .to_owned();
        let typed_val = types
            .typify(data, sem_id)
            .expect("global type doesn't match type definition");

        let serialized = types
            .strict_serialize_type::<U16>(&typed_val)
            .expect("internal error");

        builder = builder
            .add_global_state(global.name.clone(), serialized)
            .expect("invalid global state data");
    }

    // Issuer State
    let seal = ExplicitSeal::<Txid>::from_str(seal).expect("invalid seal definition");
    let seal = GenesisSeal::from(seal);

    let type_asset = TypeName::from_str("Assets").expect("invalid type_name definition");
    builder = builder
        .add_fungible_state(type_asset, seal, supply)
        .expect("invalid global state data");

    let contract = builder.issue_contract().expect("failure issuing contract");
    Ok(contract)
}
