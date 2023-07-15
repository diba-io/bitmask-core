use std::{collections::BTreeMap, fs};

use anyhow::Result;
use rgbstd::{
    interface::{LIB_ID_RGB20, LIB_ID_RGB21, LIB_ID_RGB25},
    stl::{LIB_ID_RGB, LIB_ID_RGB_CONTRACT, LIB_ID_RGB_STD},
};
use serde::{Deserialize, Serialize};

type IdVersionMap = BTreeMap<String, String>;

#[allow(non_snake_case)]
#[derive(Deserialize, Serialize)]
struct LibIds {
    LIB_ID_RGB: IdVersionMap,
    LIB_ID_RGB_CONTRACT: IdVersionMap,
    LIB_ID_RGB20: IdVersionMap,
    LIB_ID_RGB21: IdVersionMap,
    LIB_ID_RGB25: IdVersionMap,
    LIB_ID_RGB_STD: IdVersionMap,
}

const LIB_IDS_FILE: &str = "RGB_LIB_IDs.toml";
const FILE_COMMENT: &str =
    "# Auto-generated semantic IDs for RGB consensus-critical libraries and their corresponding versions of bitmask-core.\n\n";
const LIB_ID_RGB_COMMENT: &str =
    "[LIB_ID_RGB]\n# Consensus-breaking: If changed, assets must be reissued";
const LIB_ID_RGB_CONTRACT_COMMENT: &str =
    "[LIB_ID_RGB_CONTRACT]\n# Interface-only: If changed, only a new interface implementation is needed. No reiussance or migration necessary.";
const LIB_ID_RGB_STD_COMMENT: &str =
    "[LIB_ID_RGB_STD]\n# Not consensus-breaking: If changed, only stash and consignments must be updated. No reiussance or migration necessary.";

fn main() -> Result<()> {
    const BMC_VERSION: &str = env!("CARGO_PKG_VERSION");

    let toml = fs::read_to_string(LIB_IDS_FILE)?;
    let mut doc: LibIds = toml::from_str(&toml)?;

    doc.LIB_ID_RGB
        .entry(LIB_ID_RGB.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB_CONTRACT
        .entry(LIB_ID_RGB_CONTRACT.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB20
        .entry(LIB_ID_RGB20.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB21
        .entry(LIB_ID_RGB21.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB25
        .entry(LIB_ID_RGB25.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB_STD
        .entry(LIB_ID_RGB_STD.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    let toml = toml::to_string(&doc)?;
    let toml = toml.replace("[LIB_ID_RGB]", LIB_ID_RGB_COMMENT);
    let toml = toml.replace("[LIB_ID_RGB_CONTRACT]", LIB_ID_RGB_CONTRACT_COMMENT);
    let toml = toml.replace("[LIB_ID_RGB_STD]", LIB_ID_RGB_STD_COMMENT);

    fs::write(LIB_IDS_FILE, format!("{FILE_COMMENT}{toml}"))?;

    Ok(())
}
