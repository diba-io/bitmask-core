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
// For NIP-06 Nostr signing and Carbonado encryption key derivation
pub const NOSTR_PATH: &str = "m/44h/1237h/0h";

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

// carbonado
pub static CARBONADO_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("CARBONADO_ENDPOINT")));

pub async fn get_env(key: &str) -> String {
    match key {
        "LNDHUB_ENDPOINT" => LNDHUB_ENDPOINT.read().await.to_string(),
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
}
