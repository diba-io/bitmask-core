use std::{env, str::FromStr, sync::RwLock};

use anyhow::Result;
use bitcoin::Network;
use once_cell::sync::Lazy;
use tokio::sync::RwLock as AsyncRwLock;

fn dot_env(key: &str) -> String {
    let env_file = include_str!("../../.env");
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

static BITCOIN_EXPLORER_API_MAINNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_MAINNET"));
static BITCOIN_EXPLORER_API_TESTNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_TESTNET"));
static BITCOIN_EXPLORER_API_SIGNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_SIGNET"));
static BITCOIN_EXPLORER_API_REGTEST: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_REGTEST"));
pub static BITCOIN_EXPLORER_API: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(BITCOIN_EXPLORER_API_REGTEST.to_owned()));

static BITCOIN_ELECTRUM_API_MAINNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_MAINNET"));
static BITCOIN_ELECTRUM_API_TESTNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_TESTNET"));
static BITCOIN_ELECTRUM_API_SIGNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_SIGNET"));
static BITCOIN_ELECTRUM_API_REGTEST: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_REGTEST"));
pub static BITCOIN_ELECTRUM_API: Lazy<AsyncRwLock<String>> =
    Lazy::new(|| AsyncRwLock::new(BITCOIN_ELECTRUM_API_REGTEST.to_owned()));

pub static NODE_SERVER_BASE_URL: Lazy<String> = Lazy::new(|| dot_env("NODE_SERVER_BASE_URL"));

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

pub fn get_network() -> Result<String> {
    match NETWORK.read() {
        Ok(network) => Ok(network.to_string()),
        Err(err) => Ok(err.to_string()),
    }
}

/// Switch Bitcoin network
/// For options, see: https://docs.rs/bitcoin/0.27.1/src/bitcoin/network/constants.rs.html#62-75
pub async fn switch_network(network_str: &str) -> Result<()> {
    let network = Network::from_str(network_str)?;

    *BTC_PATH.write().expect("BTC_PATH is writable") = if network == Network::Bitcoin {
        BTC_MAINNET_PATH.to_owned()
    } else {
        BTC_TESTNET_PATH.to_owned()
    };

    *BITCOIN_EXPLORER_API
        .write()
        .expect("BITCOIN_EXPLORER_API is writable") = match network {
        Network::Bitcoin => BITCOIN_EXPLORER_API_MAINNET.to_owned(),
        Network::Testnet => BITCOIN_EXPLORER_API_TESTNET.to_owned(),
        Network::Signet => BITCOIN_EXPLORER_API_SIGNET.to_owned(),
        Network::Regtest => BITCOIN_EXPLORER_API_REGTEST.to_owned(),
    };

    *BITCOIN_ELECTRUM_API.write().await = match network {
        Network::Bitcoin => BITCOIN_ELECTRUM_API_MAINNET.to_owned(),
        Network::Testnet => BITCOIN_ELECTRUM_API_TESTNET.to_owned(),
        Network::Signet => BITCOIN_ELECTRUM_API_SIGNET.to_owned(),
        Network::Regtest => BITCOIN_ELECTRUM_API_REGTEST.to_owned(),
    };

    *NETWORK.write().expect("NETWORK is writable") = network;

    Ok(())
}

pub static NODE_HOST: Lazy<AsyncRwLock<String>> =
    Lazy::new(|| AsyncRwLock::new(dot_env("NODE_HOST")));

pub async fn get_endpoint(path: &str) -> String {
    let node_host = NODE_HOST.read().await;
    format!("{node_host}/{path}")
}

pub async fn switch_host(host: &str) {
    *NODE_HOST.write().await = host.to_owned();
}

// lightning
pub static LNDHUB_ENDPOINT: Lazy<String> = Lazy::new(|| dot_env("LNDHUB_ENDPOINT"));
