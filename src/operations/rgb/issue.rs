use std::str::FromStr;
use strict_types::{svstr, svstruct, StrictVal};

use super::schemas::default_fungible_schema;
use amplify::confinement::U16;
use bp::seals::txout::ExplicitSeal;
use rgbstd::containers::Contract;
use rgbstd::contract::GenesisSeal;
use rgbstd::interface::{BuilderError, ContractBuilder, Iface, IfaceImpl};
use rgbstd::Txid;
use strict_types::encoding::TypeName;

pub fn issue_contract(
    ticker: &str,
    name: &str,
    description: &str,
    precision: u8,
    supply: u64,
    seal: &str,
    iface: Iface,
    iimpl: IfaceImpl,
) -> Result<Contract, BuilderError> {
    let schema = default_fungible_schema();
    let types = &schema.type_system;

    let mut builder = ContractBuilder::with(iface, schema.clone(), iimpl.clone())
        .expect("schema fails to implement RGB20 interface");

    let global_state_data = bmap! {
        "Nominal" => svstruct!(name => name, ticker => ticker, details => StrictVal::none(), precision => precision),
        "ContractText" => svstr!(description)
    };

    // Global State
    for global in iimpl.global_state.iter() {
        let type_id = global.id;
        let type_name = global.name.as_str();
        let sem_id = schema
            .global_types
            .get(&type_id)
            .expect("invalid schema implementation")
            .sem_id;

        let data = global_state_data
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

    let type_asset = TypeName::from_str("Assets").expect("");
    builder = builder
        .add_fungible_state(type_asset, seal, supply)
        .expect("invalid global state data");

    let contract = builder.issue_contract().expect("failure issuing contract");
    Ok(contract)
}
