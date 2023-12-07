use std::{env, str::FromStr};

use anyhow::Result;
use bitcoin::Network;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::error;

fn dot_env(key: &str) -> String {
    let env_file = include_str!("../.env");
    match env::var(key) {
        Ok(val) => val,
        Err(_) => {
            if let Some(line) = env_file.split('\n').find(|e| e.starts_with(key)) {
                let (_, val) = line.split_once('=').expect("value exists for key");
                val.to_owned()
            } else {
                panic!("Couldn't access .env key: {key}");
            }
        }
    }
}

pub const BMC_VERSION: &str = env!("CARGO_PKG_VERSION");

pub static ELECTRUM_TIMEOUT: u8 = 4;

static BITCOIN_EXPLORER_API_MAINNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_MAINNET")));
static BITCOIN_EXPLORER_API_TESTNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_TESTNET")));
static BITCOIN_EXPLORER_API_SIGNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_SIGNET")));
static BITCOIN_EXPLORER_API_REGTEST: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_REGTEST")));
pub static BITCOIN_EXPLORER_API: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_REGTEST")));

static BITCOIN_ELECTRUM_API_MAINNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_MAINNET")));
static BITCOIN_ELECTRUM_API_TESTNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_TESTNET")));
static BITCOIN_ELECTRUM_API_SIGNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_SIGNET")));
static BITCOIN_ELECTRUM_API_REGTEST: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_REGTEST")));
pub static BITCOIN_ELECTRUM_API: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_REGTEST")));

pub static MARKETPLACE_SEED: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("MARKETPLACE_SEED")));

pub static MARKETPLACE_NOSTR: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("MARKETPLACE_NOSTR")));

pub static MARKETPLACE_FEE_PERC: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("MARKETPLACE_FEE_PERC")));

pub static MARKETPLACE_FEE_XPUB: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("MARKETPLACE_FEE_XPUB")));

pub async fn get_marketplace_seed() -> String {
    MARKETPLACE_SEED.read().await.to_string()
}

pub async fn get_marketplace_nostr_key() -> String {
    MARKETPLACE_NOSTR.read().await.to_string()
}

pub async fn get_marketplace_fee_percentage() -> String {
    MARKETPLACE_FEE_PERC.read().await.to_string()
}

pub async fn get_marketplace_fee_xpub() -> String {
    MARKETPLACE_FEE_XPUB.read().await.to_string()
}

pub async fn get_coordinator_nostr_key() -> String {
    MARKETPLACE_NOSTR.read().await.to_string()
}

pub static UDAS_UTXO: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(dot_env("UDAS_UTXO")));

pub async fn get_udas_utxo() -> String {
    UDAS_UTXO.read().await.to_string()
}

// Descriptor strings
pub const BTC_MAINNET_PATH: &str = "m/86h/0h/0h";
pub const BTC_TESTNET_PATH: &str = "m/86h/1h/0h";
pub static BTC_PATH: Lazy<RwLock<String>> = Lazy::new(|| {
    RwLock::new(if dot_env("BITCOIN_NETWORK") == "bitcoin" {
        BTC_MAINNET_PATH.to_owned()
    } else {
        BTC_TESTNET_PATH.to_owned()
    })
});

// Magic number for versioning descriptors
pub const DIBA_DESCRIPTOR_VERSION: u8 = 0;
pub const DIBA_MAGIC_NO: [u8; 4] = *b"DIBA";
pub const DIBA_DESCRIPTOR: [u8; 5] = [
    DIBA_MAGIC_NO[0],
    DIBA_MAGIC_NO[1],
    DIBA_MAGIC_NO[2],
    DIBA_MAGIC_NO[3],
    DIBA_DESCRIPTOR_VERSION,
];

pub static NETWORK: Lazy<RwLock<Network>> = Lazy::new(|| {
    RwLock::new(Network::from_str(&dot_env("BITCOIN_NETWORK")).expect("Parse Bitcoin network"))
});

pub async fn get_network() -> String {
    NETWORK.read().await.to_string()
}

/// Switch Bitcoin network
/// For options, see: <https://docs.rs/bitcoin/0.27.1/src/bitcoin/network/constants.rs.html#62-75>
pub async fn switch_network(network_str: &str) -> Result<()> {
    let network = Network::from_str(network_str)?;

    *BTC_PATH.write().await = if network == Network::Bitcoin {
        BTC_MAINNET_PATH.to_owned()
    } else {
        BTC_TESTNET_PATH.to_owned()
    };

    *BITCOIN_EXPLORER_API.write().await = match network {
        Network::Bitcoin => BITCOIN_EXPLORER_API_MAINNET.read().await.to_owned(),
        Network::Testnet => BITCOIN_EXPLORER_API_TESTNET.read().await.to_owned(),
        Network::Signet => BITCOIN_EXPLORER_API_SIGNET.read().await.to_owned(),
        Network::Regtest => BITCOIN_EXPLORER_API_REGTEST.read().await.to_owned(),
    };

    *BITCOIN_ELECTRUM_API.write().await = match network {
        Network::Bitcoin => BITCOIN_ELECTRUM_API_MAINNET.read().await.to_owned(),
        Network::Testnet => BITCOIN_ELECTRUM_API_TESTNET.read().await.to_owned(),
        Network::Signet => BITCOIN_ELECTRUM_API_SIGNET.read().await.to_owned(),
        Network::Regtest => BITCOIN_ELECTRUM_API_REGTEST.read().await.to_owned(),
    };

    *NETWORK.write().await = network;

    Ok(())
}

// lightning
pub static LNDHUB_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("LNDHUB_ENDPOINT")));

// bitmask node
pub static BITMASK_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITMASK_ENDPOINT")));

// rgb proxy node
pub static RGB_PROXY_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("RGB_PROXY_ENDPOINT")));

// carbonado
pub static CARBONADO_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("CARBONADO_ENDPOINT")));

pub async fn get_env(key: &str) -> String {
    match key {
        "LNDHUB_ENDPOINT" => LNDHUB_ENDPOINT.read().await.to_string(),
        "BITMASK_ENDPOINT" => BITMASK_ENDPOINT.read().await.to_string(),
        "CARBONADO_ENDPOINT" => CARBONADO_ENDPOINT.read().await.to_string(),
        "BITCOIN_EXPLORER_API_MAINNET" => BITCOIN_EXPLORER_API_MAINNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_TESTNET" => BITCOIN_EXPLORER_API_TESTNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_SIGNET" => BITCOIN_EXPLORER_API_SIGNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_REGTEST" => BITCOIN_EXPLORER_API_REGTEST.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_MAINNET" => BITCOIN_ELECTRUM_API_MAINNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_TESTNET" => BITCOIN_ELECTRUM_API_TESTNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_SIGNET" => BITCOIN_ELECTRUM_API_SIGNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_REGTEST" => BITCOIN_ELECTRUM_API_REGTEST.read().await.to_string(),
        _ => {
            error!(format!("get_env called an unknown key, {key}"));
            "".to_owned()
        }
    }
}

pub async fn set_env(key: &str, value: &str) {
    match key {
        "LNDHUB_ENDPOINT" => *LNDHUB_ENDPOINT.write().await = value.to_owned(),
        "BITMASK_ENDPOINT" => *BITMASK_ENDPOINT.write().await = value.to_owned(),
        "CARBONADO_ENDPOINT" => *CARBONADO_ENDPOINT.write().await = value.to_owned(),
        "BITCOIN_EXPLORER_API_MAINNET" => {
            *BITCOIN_EXPLORER_API_MAINNET.write().await = value.to_owned()
        }
        "BITCOIN_EXPLORER_API_TESTNET" => {
            *BITCOIN_EXPLORER_API_TESTNET.write().await = value.to_owned()
        }
        "BITCOIN_EXPLORER_API_SIGNET" => {
            *BITCOIN_EXPLORER_API_SIGNET.write().await = value.to_owned()
        }
        "BITCOIN_EXPLORER_API_REGTEST" => {
            *BITCOIN_EXPLORER_API_REGTEST.write().await = value.to_owned()
        }
        "BITCOIN_ELECTRUM_API_MAINNET" => {
            *BITCOIN_ELECTRUM_API_MAINNET.write().await = value.to_owned()
        }
        "BITCOIN_ELECTRUM_API_TESTNET" => {
            *BITCOIN_ELECTRUM_API_TESTNET.write().await = value.to_owned()
        }
        "BITCOIN_ELECTRUM_API_SIGNET" => {
            *BITCOIN_ELECTRUM_API_SIGNET.write().await = value.to_owned()
        }
        "BITCOIN_ELECTRUM_API_REGTEST" => {
            *BITCOIN_ELECTRUM_API_REGTEST.write().await = value.to_owned()
        }
        _ => {
            error!(format!("set_env called an unknown key, {key}"));
        }
    };
}

pub mod storage_keys {
    pub const ASSETS_STOCK: &str = "bitmask-fungible_assets_stock.c15";
    pub const ASSETS_WALLETS: &str = "bitmask-fungible_assets_wallets.c15";
    pub const ASSETS_TRANSFERS: &str = "bitmask_assets_transfers.c15";
    pub const ASSETS_OFFERS: &str = "bitmask-asset_offers.c15";
    pub const ASSETS_BIDS: &str = "bitmask-asset_bids.c15";
    pub const MARKETPLACE_OFFERS: &str = "bitmask-marketplace_public_offers.c15";
}
