mod accept_transaction;
mod create_psbt;
mod descriptor_wallet;
mod import_asset;
mod init;
mod issue_asset;
mod receive_tokens;
// mod send_tokens;
mod validate_transaction;

use std::str::FromStr;

pub use accept_transaction::accept_transfer;
pub use create_psbt::create_psbt;
// pub use descriptor_wallet::rgb_address;
pub use import_asset::{get_asset_by_genesis, get_assets};
pub use init::{inproc, rgb_init};
pub use issue_asset::issue_asset;
pub use receive_tokens::blind_utxo;
// pub use send_tokens::{transfer_asset, ConsignmentDetails};
pub use validate_transaction::validate_transfer;

use anyhow::{anyhow, Result};
use rgb_rpc::{client::Client, ContractValidity};
pub use rgb_std::{Contract, ContractId};

use crate::{debug, error, info};

pub fn rgb_cli() -> Result<Client> {
    use lnpbp::chain::Chain;

    let connect = inproc("rgbd");

    debug!(format!("RPC socket {}", connect));

    let chain = Chain::Testnet3;

    let mut client =
        Client::with(connect, s!("rgb-cli"), chain).expect("Error initializing client");

    if !client.hello()? {
        error!("rgbd health check failed");
        return Err(anyhow!("rgbd health check failed"));
    } else {
        debug!("rgbd health check succeeded");
    }

    Ok(client)
}

pub fn register_contract(contract_str: &str) -> Result<ContractValidity> {
    let mut client = rgb_cli()?;
    let progress = |msg: String| {
        info!("{}", msg);
    };

    let contract = Contract::from_str(contract_str)?;

    info!(format!("Registering contract {}", contract.contract_id()));

    let force = false;
    let status = client.register_contract(contract, force, progress)?;

    Ok(status)
}
