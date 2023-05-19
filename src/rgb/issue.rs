use amplify::Wrapper;
use bp::{Chain, Txid};
use rgb_schemata::{nia_rgb20, nia_schema, uda_rgb21, uda_schema};
use rgbstd::contract::GenesisSeal;
use rgbstd::interface::rgb21::{Allocation, OwnedFraction, TokenData, TokenIndex};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::stl::{DivisibleAssetSpec, Precision, RicardianContract, Timestamp};
use rgbstd::validation::ResolveTx;
use std::str::FromStr;

use rgbstd::containers::Contract;
use rgbstd::interface::{rgb20, rgb21, BuilderError, ContractBuilder};
use rgbstd::persistence::{Inventory, Stash, Stock};

use seals::txout::ExplicitSeal;
use strict_types::encoding::TypeName;

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
    resolver: &mut T,
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
        "RGB20" => issue_fungible_asset(ticker, name, description, precision, supply, seal),
        "RGB21" => issue_uda_asset(ticker, name, description, precision, supply, seal),
        _ => return Err(IssueError::ContractNotfound(iface.name.to_string())),
    };

    let resp = match contract_issued {
        Ok(resp) => resp,
        Err(err) => return Err(IssueError::Forge(err)),
    };

    let resp = match resp.clone().validate(resolver) {
        Ok(resp) => resp,
        Err(_) => return Err(IssueError::ContractInvalid(resp.contract_id().to_string())),
    };

    stock
        .import_contract(resp.clone(), resolver)
        .or(Err(IssueError::ImportContract(
            resp.contract_id().to_string(),
        )))?;

    Ok(resp)
}

/// RGB20 interface
fn issue_fungible_asset(
    ticker: &str,
    name: &str,
    _description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
) -> Result<Contract, BuilderError> {
    let iface = rgb20();
    let schema = nia_schema();
    let iimpl = nia_rgb20();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let precision = Precision::try_from(precision).expect("invalid precision");
    let spec = DivisibleAssetSpec::new(ticker, name, precision);
    let terms = RicardianContract::default();
    let created = Timestamp::default();
    // Issuer State
    let seal = ExplicitSeal::<Txid>::from_str(seal).expect("invalid seal definition");
    let seal = GenesisSeal::from(seal);

    let contract = ContractBuilder::with(iface, schema, iimpl)
        .expect("schema fails to implement RGB20 interface")
        .set_chain(Chain::Testnet3)
        .add_global_state("spec", spec)
        .expect("invalid nominal")
        .add_global_state("created", created)
        .expect("invalid nominal")
        .add_global_state("terms", terms)
        .expect("invalid contract text")
        .add_fungible_state("assetOwner", seal, supply)
        .expect("invalid asset amount")
        .issue_contract()
        .expect("contract doesn't fit schema requirements");
    Ok(contract)
}

/// RGB21 interface
fn issue_uda_asset(
    ticker: &str,
    name: &str,
    _description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
) -> Result<Contract, BuilderError> {
    let iface = rgb21();
    let schema = uda_schema();
    let iimpl = uda_rgb21();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let precision = Precision::try_from(precision).expect("invalid precision");
    let spec = DivisibleAssetSpec::new(ticker, name, precision);
    let terms = RicardianContract::default();
    let created = Timestamp::default();

    let fraction = OwnedFraction::from_inner(supply);
    let index = TokenIndex::from_inner(1);

    let token_data = TokenData {
        index,
        ..Default::default()
    };

    // Issuer State
    let seal = ExplicitSeal::<Txid>::from_str(seal).expect("invalid seal definition");
    let seal = GenesisSeal::from(seal);
    let allocation = Allocation::with(index, fraction);

    let contract = ContractBuilder::with(iface, schema, iimpl)
        .expect("schema fails to implement RGB21 interface")
        .set_chain(Chain::Testnet3)
        .add_global_state("spec", spec)
        .expect("invalid nominal")
        .add_global_state("created", created)
        .expect("invalid nominal")
        .add_global_state("terms", terms)
        .expect("invalid contract text")
        .add_global_state("tokens", token_data)
        .expect("invalid nominal")
        .add_data_state("assetOwner", seal, allocation)
        .expect("invalid asset blob")
        .issue_contract()
        .expect("contract doesn't fit schema requirements");
    Ok(contract)
}
