use amplify::confinement::SmallBlob;
use amplify::Wrapper;
use bp::seals::txout::ExplicitSeal;
use bp::{Chain, Txid};
use rgb_schemata::{nia_rgb20, nia_schema, uda_rgb21, uda_schema};
use rgbstd::contract::GenesisSeal;
use rgbstd::interface::rgb21::{Allocation, EmbeddedMedia, OwnedFraction, TokenData, TokenIndex};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::stl::{
    DivisibleAssetSpec, MediaType, Name, Precision, RicardianContract, Ticker, Timestamp,
};
use rgbstd::validation::ResolveTx;
use std::str::FromStr;

use rgbstd::containers::Contract;
use rgbstd::interface::{rgb20, rgb21, BuilderError, ContractBuilder};
use rgbstd::persistence::{Inventory, Stash, Stock};

use crate::structs::{IssueMetaRequest, IssueMetadata};
// use seals::txout::ExplicitSeal;
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
    network: &str,
    meta: IssueMetaRequest,
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
        "RGB20" => {
            issue_fungible_asset(ticker, name, description, precision, supply, seal, network)
        }
        "RGB21" => issue_uda_asset(
            ticker,
            name,
            description,
            precision,
            supply,
            seal,
            network,
            meta,
        ),
        _ => return Err(IssueError::ContractNotfound(iface.name.to_string())),
    };

    let resp = match contract_issued {
        Ok(resp) => resp,
        Err(err) => return Err(IssueError::Forge(err)),
    };

    let resp = match resp.clone().validate(resolver) {
        Ok(resp) => resp,
        Err(_err) => return Err(IssueError::ContractInvalid(resp.contract_id().to_string())),
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
    description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
    network: &str,
) -> Result<Contract, BuilderError> {
    let iface = rgb20();
    let schema = nia_schema();
    let iimpl = nia_rgb20();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let description: &'static str = Box::leak(description.to_string().into_boxed_str());
    let precision = Precision::try_from(precision).expect("invalid precision");
    let spec = DivisibleAssetSpec::new(ticker, name, precision);
    let terms = RicardianContract::new(description);
    let created = Timestamp::default();
    // Issuer State
    let seal = ExplicitSeal::<Txid>::from_str(seal).expect("invalid seal definition");
    let seal = GenesisSeal::from(seal);

    let contract = ContractBuilder::with(iface, schema, iimpl)
        .expect("schema fails to implement RGB20 interface")
        .set_chain(Chain::from_str(network).expect("invalid network"))
        .add_global_state("spec", spec)
        .expect("invalid spec")
        .add_global_state("created", created)
        .expect("invalid created")
        .add_global_state("terms", terms)
        .expect("invalid contract text")
        .add_fungible_state("beneficiary", seal, supply)
        .expect("invalid asset amount")
        .issue_contract()
        .expect("contract doesn't fit schema requirements");
    Ok(contract)
}

/// RGB21 interface
#[allow(clippy::too_many_arguments)]
fn issue_uda_asset(
    ticker: &str,
    name: &str,
    description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
    network: &str,
    meta: IssueMetaRequest,
) -> Result<Contract, BuilderError> {
    let iface = rgb21();
    let schema = uda_schema();
    let iimpl = uda_rgb21();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let description: &'static str = Box::leak(description.to_string().into_boxed_str());
    let precision = Precision::try_from(precision).expect("invalid precision");
    let spec = DivisibleAssetSpec::new(ticker, name, precision);
    let terms = RicardianContract::new(description);
    let created = Timestamp::default();
    let fraction = OwnedFraction::from_inner(supply);

    let mut tokens_data = vec![];
    let mut allocations = vec![];

    // Toke Data
    let mut token_index = 1;
    let IssueMetaRequest(issue_meta) = meta;
    for meta in issue_meta {
        match meta {
            IssueMetadata::UDA(uda) => {
                let index = TokenIndex::from_inner(1);
                let media_ty: &'static str = Box::leak(uda.ty.to_string().into_boxed_str());
                let preview = Some(EmbeddedMedia {
                    ty: MediaType::with(media_ty),
                    data: SmallBlob::try_from_iter(uda.source.as_bytes().to_vec())
                        .expect("invalid data"),
                });
                let token_data = TokenData {
                    index,
                    name: Some(spec.clone().naming.name),
                    ticker: Some(spec.clone().naming.ticker),
                    preview,
                    ..Default::default()
                };

                let allocation = Allocation::with(index, fraction);
                tokens_data.push(token_data);
                allocations.push(allocation);
            }
            IssueMetadata::Collectible(items) => {
                for item in items {
                    let index = TokenIndex::from_inner(token_index);
                    let media_ty: &'static str =
                        Box::leak(item.preview.ty.to_string().into_boxed_str());
                    let preview = Some(EmbeddedMedia {
                        ty: MediaType::with(media_ty),
                        data: SmallBlob::try_from_iter(item.preview.source.as_bytes().to_vec())
                            .expect("invalid data"),
                    });
                    let token_data = TokenData {
                        index,
                        name: Some(Name::from_str(&item.name).expect("invalid name")),
                        ticker: Some(Ticker::from_str(&item.name).expect("invalid ticker")),
                        preview,
                        ..Default::default()
                    };

                    let allocation = Allocation::with(index, fraction);
                    tokens_data.push(token_data);
                    allocations.push(allocation);
                    token_index += 1;
                }
            }
        }
    }

    let seal = ExplicitSeal::<Txid>::from_str(seal).expect("invalid seal definition");
    let seal = GenesisSeal::from(seal);

    let mut contract = ContractBuilder::with(iface, schema, iimpl)
        .expect("schema fails to implement RGB21 interface")
        .set_chain(Chain::from_str(network).expect("invalid network"))
        .add_global_state("spec", spec)
        .expect("invalid spec")
        .add_global_state("created", created)
        .expect("invalid created")
        .add_global_state("terms", terms)
        .expect("invalid contract text");

    for token_data in tokens_data {
        contract = contract
            .add_global_state("tokens", token_data)
            .expect("invalid tokens");
    }

    for allocation in allocations {
        contract = contract
            .add_data_state("beneficiary", seal, allocation)
            .expect("invalid asset blob");
    }

    let contract = contract
        .issue_contract()
        .expect("contract doesn't fit schema requirements");
    Ok(contract)
}
