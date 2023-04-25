use aluvm::library::Lib;
use aluvm::library::LibSite;
use amplify::{confined_bmap, tiny_bmap};
use rgbstd::interface::rgb20;
use rgbstd::interface::IfaceImpl;
use rgbstd::interface::NamedType;
use rgbstd::schema::FungibleType;
use rgbstd::schema::GlobalStateSchema;
use rgbstd::schema::Occurrences;
use rgbstd::schema::StateSchema;
use rgbstd::schema::TransitionSchema;
use rgbstd::vm::EntryPoint;
use rgbstd::{
    schema::{GenesisSchema, Schema, Script, SubSchema},
    stl::StandardTypes,
    vm::{AluScript, ContractOp, RgbIsa},
};
use strict_types::tn;
use strict_types::{SemId, Ty};

const GS_NOMINAL: u16 = 0;
const GS_CONTRACT: u16 = 1;
const OS_ASSETS: u16 = 0;
const TS_TRANSFER: u16 = 0;

/// Default schema used to fungible contracts
pub fn default_fungible_schema() -> SubSchema {
    let types = StandardTypes::new();

    let code = [RgbIsa::Contract(ContractOp::PcVs(OS_ASSETS))];
    let alu_lib = Lib::assemble(&code).expect("assemble library from bytecode");
    let alu_id = alu_lib.id();

    Schema {
        ffv: zero!(),
        subset_of: None,
        type_system: types.type_system(),
        global_types: tiny_bmap! {
            GS_NOMINAL => GlobalStateSchema::once(types.get("RGBContract.Nominal")),
            GS_CONTRACT => GlobalStateSchema::once(types.get("RGBContract.ContractText")),
        },
        owned_types: tiny_bmap! {
            OS_ASSETS => StateSchema::Fungible(FungibleType::Unsigned64Bit),
        },
        valency_types: none!(),
        genesis: GenesisSchema {
            metadata: Ty::<SemId>::UNIT.id(None),
            globals: tiny_bmap! {
                GS_NOMINAL => Occurrences::Once,
                GS_CONTRACT => Occurrences::Once,
            },
            assignments: tiny_bmap! {
                OS_ASSETS => Occurrences::OnceOrMore,
            },
            valencies: none!(),
        },
        extensions: none!(),
        transitions: tiny_bmap! {
            TS_TRANSFER => TransitionSchema {
                metadata: Ty::<SemId>::UNIT.id(None),
                globals: none!(),
                inputs: tiny_bmap! {
                    OS_ASSETS => Occurrences::OnceOrMore
                },
                assignments: tiny_bmap! {
                    OS_ASSETS => Occurrences::OnceOrMore
                },
                valencies: none!(),
            }
        },
        script: Script::AluVM(AluScript {
            libs: confined_bmap! { alu_id => alu_lib },
            entry_points: confined_bmap! {
                EntryPoint::ValidateOwnedState(OS_ASSETS) => LibSite::with(0, alu_id)
            },
        }),
    }
}

/// Default Schema + Official RGB20 Interface
pub fn default_fungible_iimpl() -> IfaceImpl {
    let schema = default_fungible_schema();
    let iface = rgb20();

    IfaceImpl {
        schema_id: schema.schema_id(),
        iface_id: iface.iface_id(),
        global_state: tiny_bset! {
            NamedType::with(GS_NOMINAL, tn!("Nominal")),
            NamedType::with(GS_CONTRACT, tn!("ContractText")),
        },
        assignments: tiny_bset! {
            NamedType::with(OS_ASSETS, tn!("Assets")),
        },
        valencies: none!(),
        transitions: tiny_bset! {
            NamedType::with(TS_TRANSFER, tn!("Transfer")),
        },
        extensions: none!(),
        version: rgbstd::interface::VerNo::V1,
    }
}
