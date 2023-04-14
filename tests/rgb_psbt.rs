#![cfg(not(target_arch = "wasm32"))]
use std::str::FromStr;

use bitcoin::EcdsaSighashType;
use bitmask_core::operations::rgb::psbt::create_psbt;
use miniscript_crate::Descriptor;
use psbt::{ProprietaryKeyDescriptor, ProprietaryKeyLocation, ProprietaryKeyType};
use wallet::{
    descriptors::InputDescriptor,
    hd::{DerivationAccount, UnhardenedIndex},
};

mod rgb_test_utils;
use rgb_test_utils::DumbResolve;

#[tokio::test]
async fn allow_create_input_descriptor() -> anyhow::Result<()> {
    let input = InputDescriptor {
        outpoint: "9a035b0e6e9d07065a31c49884cb1c2d8953636346e91948df75b20e27f50f24:8"
            .parse()
            .unwrap(),
        terminal: "/0/0".parse().unwrap(),
        seq_no: "time(0)".parse().unwrap(),
        tweak: None,
        sighash_type: EcdsaSighashType::All,
    };

    let expected =
        "9a035b0e6e9d07065a31c49884cb1c2d8953636346e91948df75b20e27f50f24:8 /0/0 time(0)";

    assert_eq!(expected, input.to_string());
    Ok(())
}

#[tokio::test]
async fn allow_create_tapret_proprieties() -> anyhow::Result<()> {
    let prop = ProprietaryKeyDescriptor {
        location: ProprietaryKeyLocation::Output(0),
        ty: ProprietaryKeyType {
            prefix: "TAPRET".to_owned(),
            subtype: 0,
        },
        key: None,
        value: None,
    };
    let expected = "output(0) TAPRET(0)";

    assert_eq!(expected, prop.to_string().trim());
    Ok(())
}

#[tokio::test]
async fn allow_create_psbt_file() -> anyhow::Result<()> {
    let desc = "tr(m=[280a5963]/86h/1h/0h=[tpubDCa3US185mM8yGTXtPWY1wNRMCiX89kzN4dwTMKUJyiJnnq486MTeyYShvHiS8Dd1zR2myy5xyJFDs5YacVHn6JZbVaDAtkrXZE3tTVRHPu]/*/*)#8an50cqp";
    let input_desc =
        "21bfd80759b8d1ce6d67570fb40a53c50b167f4ffef14b87c2ee1984cb5c44d1:1 /0/0 time(0)";
    let props = "output(0) TAPRET(0) ";

    let descriptor: Descriptor<DerivationAccount> = Descriptor::from_str(desc).expect("");
    let lock_time = 0.into();
    let inputs = vec![InputDescriptor::from_str(input_desc).expect("")];
    let outputs = vec![];
    let proprietary_keys = vec![ProprietaryKeyDescriptor::from_str(props).expect("")];
    let change_index = UnhardenedIndex::default();
    let fee = 1000;
    let tx_resolver = DumbResolve {};

    let psbt = create_psbt(
        &descriptor,
        lock_time,
        inputs,
        outputs,
        proprietary_keys,
        change_index,
        fee,
        &tx_resolver,
    );

    assert!(psbt.is_ok());

    Ok(())
}
