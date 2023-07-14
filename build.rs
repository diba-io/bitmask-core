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
    LIB_ID_RGB20: IdVersionMap,
    LIB_ID_RGB21: IdVersionMap,
    LIB_ID_RGB25: IdVersionMap,
    LIB_ID_RGB_CONTRACT: IdVersionMap,
    LIB_ID_RGB_STD: IdVersionMap,
}

const LIB_IDS_FILE: &str = "RGB_LIB_IDs.toml";
const NOTE_COMMENT: &str =
    "# Auto-generated semantic IDs for RGB consensus-critical libraries and their corresponding versions of bitmask-core.\n\n";

fn main() -> Result<()> {
    const BMC_VERSION: &str = env!("CARGO_PKG_VERSION");

    let lib_ids_file = fs::read_to_string(LIB_IDS_FILE)?;
    let mut lib_ids: LibIds = toml::from_str(&lib_ids_file)?;

    lib_ids
        .LIB_ID_RGB
        .entry(LIB_ID_RGB.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    lib_ids
        .LIB_ID_RGB20
        .entry(LIB_ID_RGB20.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    lib_ids
        .LIB_ID_RGB21
        .entry(LIB_ID_RGB21.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    lib_ids
        .LIB_ID_RGB25
        .entry(LIB_ID_RGB25.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    lib_ids
        .LIB_ID_RGB_CONTRACT
        .entry(LIB_ID_RGB_CONTRACT.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    lib_ids
        .LIB_ID_RGB_STD
        .entry(LIB_ID_RGB_STD.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    let toml = toml::to_string(&lib_ids)?;
    fs::write(LIB_IDS_FILE, format!("{NOTE_COMMENT}{toml}"))?;

    Ok(())
}
