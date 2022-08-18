use anyhow::Result;
use std::{env, str::FromStr, sync::RwLock};
use tokio::sync::RwLock as AsyncRwLock;

use bitcoin::Network;
use once_cell::sync::Lazy;

fn dot_env(key: &str) -> String {
    let env_file = include_str!("../../.env");
    match env::var(key) {
        Ok(val) => val,
        Err(_) => {
            if let Some(line) = env_file.split('\n').find(|e| e.starts_with(key)) {
                let (_, val) = line.split_once('=').expect("value exists for key");
                val.to_owned()
            } else {
                panic!("Couldn't access .env key: {}", key);
            }
        }
    }
}

static BITCOIN_EXPLORER_API_MAINNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_MAINNET"));
static BITCOIN_EXPLORER_API_TESTNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_TESTNET"));
static BITCOIN_EXPLORER_API_SIGNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_EXPLORER_API_SIGNET"));
pub static BITCOIN_EXPLORER_API: Lazy<RwLock<String>> = Lazy::new(|| {
    RwLock::new(BITCOIN_EXPLORER_API_TESTNET.to_owned()) //TODO: Change default to mainnet
});

static BITCOIN_ELECTRUM_API_MAINNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_MAINNET"));
static BITCOIN_ELECTRUM_API_TESTNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_TESTNET"));
static BITCOIN_ELECTRUM_API_SIGNET: Lazy<String> =
    Lazy::new(|| dot_env("BITCOIN_ELECTRUM_API_SIGNET"));
pub static BITCOIN_ELECTRUM_API: Lazy<AsyncRwLock<String>> = Lazy::new(|| {
    AsyncRwLock::new(BITCOIN_ELECTRUM_API_TESTNET.to_owned()) //TODO: Change default to mainnet
});

pub static NODE_SERVER_BASE_URL: Lazy<String> = Lazy::new(|| dot_env("NODE_SERVER_BASE_URL"));

// Descriptor strings
// For SATS
pub const BTC_PATH: &str = "m/86h/1h/0h/0";
pub const BTC_CHANGE_PATH: &str = "m/86h/1h/0h/1";
// For TOKENS ---> that's provisional, it will be replace for RGB final guidelines
pub const RGB_TOKENS_PATH: &str = "m/168h/20h/0h/0h";
// For UDAS ---> that's provisional, it will be replace for RGB final guidelines
pub const RGB_NFTS_PATH: &str = "m/168h/21h/0h/0h";

pub static NETWORK: Lazy<RwLock<Network>> = Lazy::new(|| {
    RwLock::new(Network::Testnet) // TODO: Change default to mainnet
});

// See: https://docs.rs/bitcoin/0.27.1/src/bitcoin/network/constants.rs.html#62-75
pub async fn switch_network(network_str: &str) -> Result<()> {
    let network = Network::from_str(network_str).unwrap();

    *BITCOIN_EXPLORER_API.write().unwrap() = match network {
        Network::Bitcoin => BITCOIN_EXPLORER_API_MAINNET.to_owned(),
        Network::Testnet => BITCOIN_EXPLORER_API_TESTNET.to_owned(),
        Network::Signet => BITCOIN_EXPLORER_API_SIGNET.to_owned(),
        Network::Regtest => unimplemented!(),
    };

    *BITCOIN_ELECTRUM_API.write().await = match network {
        Network::Bitcoin => BITCOIN_ELECTRUM_API_MAINNET.to_owned(),
        Network::Testnet => BITCOIN_ELECTRUM_API_TESTNET.to_owned(),
        Network::Signet => BITCOIN_ELECTRUM_API_SIGNET.to_owned(),
        Network::Regtest => unimplemented!(),
    };

    *NETWORK.write().unwrap() = network;

    Ok(())
}

/// Get a node URL, using the default node URL is none is provided
pub fn url(path: &str, node_url: &Option<String>) -> String {
    let node_url = match node_url {
        Some(url) => url,
        None => &NODE_SERVER_BASE_URL,
    };
    format!("{node_url}{path}")
}
