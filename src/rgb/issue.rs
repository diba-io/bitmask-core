use amplify::{
    confinement::{Confined, SmallBlob},
    Wrapper,
};
use bp::{seals::txout::ExplicitSeal, Chain, Txid};
use rgb_schemata::{nia_rgb20, nia_schema, uda_rgb21, uda_schema};
use rgbstd::{
    containers::Contract,
    contract::GenesisSeal,
    interface::{
        rgb20, rgb21,
        rgb21::{Allocation, EmbeddedMedia, OwnedFraction, TokenData, TokenIndex},
        BuilderError, ContractBuilder,
    },
    persistence::{Inventory, Stash, Stock},
    resolvers::ResolveHeight,
    stl::{
        Amount, Attachment, ContractData, DivisibleAssetSpec, MediaType, Precision,
        RicardianContract, Timestamp,
    },
    validation::{Failure, ResolveTx},
};
use std::str::FromStr;
use strict_types::encoding::TypeName;

use crate::structs::IssueMediaRequest;

#[derive(Clone, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum IssueContractError {
    Issue(IssueError),
    Forge(BuilderError),
    /// The contract interface {0} is not supported in issuer operation
    NoContractSupport(String),
    /// The contract {0} contains failures {1:?}
    ContractInvalid(String, Vec<Failure>),
    /// The contract {0} cannot be imported (reason: {1})
    NoImport(String, String),
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
    meta: Option<IssueMediaRequest>,
    resolver: &mut T,
    stock: &mut Stock,
) -> Result<Contract, IssueContractError>
where
    T: ResolveHeight + ResolveTx,
    T::Error: 'static,
{
    let iface_name = TypeName::from_str(iface)
        .map_err(|_| IssueContractError::Forge(BuilderError::InterfaceMismatch))?;

    let iface = stock
        .iface_by_name(&iface_name)
        .map_err(|_| IssueContractError::Forge(BuilderError::InterfaceMismatch))?;

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
        _ => {
            return Err(IssueContractError::NoContractSupport(
                iface.name.to_string(),
            ))
        }
    };

    let resp = contract_issued.map_err(IssueContractError::Issue)?;
    let contract_id = resp.contract_id().to_string();
    let resp = resp.validate(resolver).map_err(|consig| {
        IssueContractError::ContractInvalid(
            contract_id.clone(),
            consig.into_validation_status().unwrap_or_default().failures,
        )
    })?;

    stock
        .import_contract(resp.clone(), resolver)
        .map_err(|err| IssueContractError::NoImport(contract_id, err.to_string()))?;

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
) -> Result<Contract, IssueError> {
    let iface = rgb20();
    let schema = nia_schema();
    let iimpl = nia_rgb20();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let description: &'static str = Box::leak(description.to_string().into_boxed_str());
    let created = Timestamp::now();

    let precision = Precision::try_from(precision).expect("invalid precision");
    let spec = DivisibleAssetSpec::new(ticker, name, precision);
    let terms = RicardianContract::from_str(description).expect("invalid contract text");
    let contract_data = ContractData { terms, media: None };

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
        .add_global_state("data", contract_data)
        .expect("invalid contract text")
        .add_global_state("issuedSupply", Amount::from(supply))
        .expect("invalid issued supply")
        .add_fungible_state("assetOwner", seal, supply)
        .expect("invalid asset amount")
        .issue_contract()
        .expect("contract doesn't fit schema requirements");
    Ok(contract)
}

#[derive(Clone, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum IssueError {
    RgbError(#[from] BuilderError),

    HexError(#[from] hex::FromHexError),
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
    meta: Option<IssueMediaRequest>,
) -> Result<Contract, IssueError> {
    let iface = rgb21();
    let schema = uda_schema();
    let iimpl = uda_rgb21();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let description: &'static str = Box::leak(description.to_string().into_boxed_str());
    let precision = Precision::try_from(precision).expect("invalid precision");
    let spec = DivisibleAssetSpec::new(ticker, name, precision);
    let terms = RicardianContract::from_str(description).expect("invalid terms");
    let created = Timestamp::now();
    let fraction = OwnedFraction::from_inner(supply);

    let mut tokens_data = vec![];
    let mut allocations = vec![];

    // Toke Data
    let token_index = TokenIndex::from_inner(0);
    if let Some(media_data) = meta {
        // Preview
        let preview = if let Some(media_preview) = media_data.preview {
            let ty_preview: &'static str = Box::leak(media_preview.ty.to_string().into_boxed_str());
            let preview = base64::decode(&media_preview.source).expect("invalid preview data");

            Some(EmbeddedMedia {
                ty: MediaType::with(ty_preview),
                data: SmallBlob::try_from_iter::<Vec<u8>>(preview).expect("invalid preview data"),
            })
        } else {
            None
        };

        // Media
        let media = if let Some(media) = media_data.media {
            let mut digest: [u8; 32] = [0; 32];
            digest.copy_from_slice(&hex::decode(&media.source)?);
            let ty: &'static str = Box::leak(media.ty.to_string().into_boxed_str());

            Some(Attachment {
                ty: MediaType::with(ty),
                digest,
            })
        } else {
            None
        };

        // Attachments
        let mut attachments = bmap![];
        for (index, attach) in media_data.attachments.iter().enumerate() {
            let mut digest: [u8; 32] = [0; 32];
            digest.copy_from_slice(&hex::decode(&attach.source)?);
            let ty: &'static str = Box::leak(attach.ty.to_string().into_boxed_str());

            attachments.insert(
                index as u8,
                Attachment {
                    ty: MediaType::with(ty),
                    digest,
                },
            );
        }

        let attachments = Confined::from_collection_unsafe(attachments);
        let naming = spec.naming.clone();
        let token_data = TokenData {
            index: token_index,
            name: Some(naming.name),
            ticker: Some(naming.ticker),
            preview,
            media,
            attachments,
            ..Default::default()
        };

        let allocation = Allocation::with(token_index, fraction);
        tokens_data.push(token_data);
        allocations.push(allocation);
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
            .add_data_state("assetOwner", seal, allocation)
            .expect("invalid asset blob");
    }

    let contract = contract
        .issue_contract()
        .expect("contract doesn't fit schema requirements");
    Ok(contract)
}
