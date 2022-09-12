use std::str::FromStr;

use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use psbt::Psbt;
use rgb_core::SealEndpoint;
use rgb_rpc::{client::Client as RgbClient, ContractValidity, TransferFinalize};
use rgb_std::{Contract, ContractId, InmemConsignment, TransferConsignment};

use crate::{debug, error, info, operations::rgb::inproc};

pub fn rgb_cli() -> Result<RgbClient> {
    use lnpbp::chain::Chain;

    let connect = inproc("rgbd");
    let chain = Chain::Testnet3; // TODO: Determine at runtime

    debug!(format!("RPC socket {connect}"));
    debug!(format!("Chain {chain}"));

    let mut rgb_client =
        RgbClient::with(connect, s!("rgb-cli"), chain).expect("Error initializing client");

    if !rgb_client.hello()? {
        error!("rgbd health check failed");
        return Err(anyhow!("rgbd health check failed"));
    } else {
        debug!("rgbd health check succeeded");
    }

    Ok(rgb_client)
}

fn progress(msg: String) {
    info!(msg);
}

pub fn register_contract(
    rgb_client: &mut RgbClient,
    contract_str: &str,
) -> Result<(ContractValidity, ContractId)> {
    let contract = Contract::from_str(contract_str)?;
    let contract_id = contract.contract_id();
    info!(format!("Registering contract {}", contract_id));

    let force = false;
    let status = rgb_client.register_contract(contract, force, progress)?;

    Ok((status, contract_id))
}

pub fn transfer_compose(
    rgb_client: &mut RgbClient,
    node_types: Vec<u16>,
    contract_id: ContractId,
    outpoints: Vec<OutPoint>,
) -> Result<InmemConsignment<TransferConsignment>> {
    let transfer = rgb_client.consign(
        contract_id,
        node_types,
        outpoints.into_iter().collect(),
        progress,
    )?;
    Ok(transfer)
}

pub fn transfer_finalize(
    rgb_client: &mut RgbClient,
    psbt: Psbt,
    consignment: InmemConsignment<TransferConsignment>,
    endseals: Vec<SealEndpoint>,
) -> Result<TransferFinalize> {
    let transfer = rgb_client.transfer(consignment, endseals, psbt, None, progress)?;
    Ok(transfer)
}
