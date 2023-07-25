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

type HashNameMap = BTreeMap<String, String>;

#[allow(non_snake_case)]
#[derive(Deserialize, Serialize)]
struct FileHashes {
    ASSETS_STOCK: HashNameMap,
    ASSETS_WALLETS: HashNameMap,
}

const FILE_HASHES_FILE: &str = "file_hashes.toml";
const ASSETS_STOCK: &str = "bitmask-fungible_assets_stock.c15";
const ASSETS_WALLETS: &str = "bitmask-fungible_assets_wallets.c15";
const NETWORK: &str = "bitcoin"; // Only mainnet is tracked, no monetary incentive to upgrade testnet assets

fn main() -> Result<()> {
    // lib ids
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

    // file hashes
    let toml = fs::read_to_string(FILE_HASHES_FILE)?;
    let mut doc: FileHashes = toml::from_str(&toml)?;

    let assets_stock_name = format!("{LIB_ID_RGB}-{ASSETS_STOCK}");
    let assets_wallets_name = format!("{LIB_ID_RGB}-{ASSETS_WALLETS}");

    let assets_stock_hash = blake3::hash(assets_stock_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_wallets_hash = blake3::hash(assets_wallets_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    doc.ASSETS_STOCK
        .entry(format!("{NETWORK}-{assets_stock_hash}.c15"))
        .or_insert(assets_stock_name);

    doc.ASSETS_WALLETS
        .entry(format!("{NETWORK}-{assets_wallets_hash}.c15"))
        .or_insert(assets_wallets_name);

    let toml = toml::to_string(&doc)?;

    fs::write(FILE_HASHES_FILE, toml)?;

    Ok(())
}
