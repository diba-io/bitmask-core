use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use amplify::hex::ToHex;
use anyhow::anyhow;
use bitcoin::{OutPoint, Script, Txid};
use bitcoin_30::{bip32::ExtendedPubKey, ScriptBuf};
use bitcoin_scripts::{
    address::{AddressCompat, AddressNetwork},
    PubkeyScript,
};
use rgb::{DeriveInfo, MiningStatus, Resolver, RgbDescr, RgbWallet, SpkDescriptor, Tapret, Utxo};
use rgbstd::{
    contract::ContractId,
    persistence::{Inventory, Stash, Stock},
    validation::ResolveTx,
};
use strict_encoding::tn;

use crate::{
    debug,
    rgb::{resolvers::ResolveSpent, structs::AddressTerminal},
    structs::{
        AllocationDetail, AllocationValue, TxStatus, UDAPosition, UtxoSpentStatus, WatcherDetail,
    },
};

pub fn create_wallet(
    iface: &str,
    xpub: ExtendedPubKey,
    wallets: &mut HashMap<String, RgbWallet>,
) -> Result<RgbWallet, anyhow::Error> {
    let descr = RgbDescr::Tapret(Tapret {
        xpub,
        taprets: empty!(),
    });

    let wallet = RgbWallet {
        descr,
        utxos: empty!(),
    };

    wallets.insert(iface.to_string(), wallet.clone());
    Ok(wallet)
}

pub fn list_addresses(
    iface_index: u32,
    wallet: RgbWallet,
    network: AddressNetwork,
) -> Result<Vec<AddressTerminal>, anyhow::Error> {
    let derives: Vec<DeriveInfo> = wallet
        .utxos
        .into_iter()
        .map(|utxo| utxo.derivation)
        .collect();

    let max = derives
        .into_iter()
        .map(|d| d.terminal.index)
        .max()
        .unwrap_or_default();

    let scripts = wallet.descr.derive(iface_index, 0..max);

    Ok(scripts
        .into_iter()
        .map(|(d, sb)| {
            let sc = Script::from_str(&sb.to_hex()).expect("invalid script data");
            let address =
                AddressCompat::from_script(&sc.into(), network).expect("invalid address data");
            let terminal = d.terminal;
            AddressTerminal { address, terminal }
        })
        .collect())
}

pub fn list_utxos(wallet: RgbWallet) -> Result<Vec<Utxo>, anyhow::Error> {
    Ok(wallet.utxos.into_iter().collect())
}

pub fn get_address(
    iface_index: u32,
    index: u32,
    wallet: RgbWallet,
    network: AddressNetwork,
) -> Result<AddressTerminal, anyhow::Error> {
    let index = index + 1;
    let scripts = wallet.descr.derive(iface_index, 0..index);
    let addresses: Vec<AddressTerminal> = scripts
        .into_iter()
        .map(|(d, sb)| {
            let sc = Script::from_str(&sb.to_hex_string()).expect("invalid script data");
            let address =
                AddressCompat::from_script(&sc.into(), network).expect("invalid address data");
            let terminal = d.terminal;
            AddressTerminal { address, terminal }
        })
        .collect();

    debug!(format!("RGB Addresses: {addresses:?}"));

    Ok(addresses[addresses.len() - 1].clone())
}

pub fn next_address(
    iface_index: u32,
    wallet: RgbWallet,
    network: AddressNetwork,
) -> Result<AddressTerminal, anyhow::Error> {
    let derives: Vec<DeriveInfo> = wallet
        .utxos
        .into_iter()
        .filter(|utxo| utxo.derivation.terminal.app == iface_index)
        .map(|utxo| utxo.derivation)
        .collect();

    let max = derives
        .into_iter()
        .map(|d| d.terminal.index)
        .max()
        .unwrap_or_default();

    debug!(format!("Max RGB wallet derivations: {max}"));

    let next_index = max + 1;
    let scripts = wallet.descr.derive(iface_index, max..next_index);
    let addresses: Vec<AddressTerminal> = scripts
        .into_iter()
        .map(|(d, sb)| {
            let sc = Script::from_str(&sb.to_hex_string()).expect("invalid script data");
            let address =
                AddressCompat::from_script(&sc.into(), network).expect("invalid address data");
            let terminal = d.terminal;
            AddressTerminal { address, terminal }
        })
        .collect();

    debug!(format!("RGB Addresses: {addresses:?}"));

    Ok(addresses[addresses.len() - 1].clone())
}

pub fn next_utxo(
    iface_index: u32,
    wallet: RgbWallet,
    resolver: &mut impl ResolveSpent,
) -> Result<Option<Utxo>, anyhow::Error> {
    let utxos: Vec<Utxo> = wallet
        .utxos
        .into_iter()
        .filter(|utxo| utxo.derivation.terminal.app == iface_index)
        .collect();

    if utxos.is_empty() {
        return Ok(none!());
    }

    let mut h_min = 0;
    let mut next_utxo = none!();
    for utxo in utxos {
        let txid =
            Txid::from_str(&utxo.outpoint.txid.to_hex()).expect("invalid transaction id parse");
        let index = utxo.outpoint.vout.into_u32();
        let utxo_status = resolver
            .resolve_spent_status(txid, index.into(), true)
            .expect("unavaliable service");
        if !utxo_status.is_spent && !utxo_status.clone().is_invalid_state() {
            match utxo_status.block_height {
                TxStatus::Mempool => {
                    if h_min == 0 {
                        next_utxo = Some(utxo);
                    }
                }
                TxStatus::Block(h_current) => {
                    if h_min == 0 || h_current < h_min {
                        h_min = h_current;
                        next_utxo = Some(utxo);
                    }
                }
                _ => return Err(anyhow!("Unexpected behavior when fetching UTXO data")),
            }
        }
    }

    Ok(next_utxo)
}

pub fn next_utxos(
    iface_index: u32,
    wallet: RgbWallet,
    resolver: &mut impl ResolveSpent,
) -> Result<Vec<Utxo>, anyhow::Error> {
    let utxos: Vec<Utxo> = wallet
        .utxos
        .into_iter()
        .filter(|utxo| utxo.derivation.terminal.app == iface_index)
        .collect();

    if utxos.is_empty() {
        return Ok(vec![]);
    }

    let mut next_utxo: Vec<Utxo> = vec![];
    for utxo in utxos {
        let txid =
            Txid::from_str(&utxo.outpoint.txid.to_hex()).expect("invalid transaction id parse");
        let index = utxo.outpoint.vout.into_u32();
        let utxo_status = resolver
            .resolve_spent_status(txid, index.into(), true)
            .expect("unavaliable service");
        if !utxo_status.is_spent && !utxo_status.is_invalid_state() && !next_utxo.contains(&utxo) {
            next_utxo.push(utxo);
        }
    }
    Ok(next_utxo)
}

pub fn sync_wallet(iface_index: u32, wallet: &mut RgbWallet, resolver: &mut impl Resolver) {
    let step = 20;
    let index = 0;

    let new_scripts = wallet.descr.derive(iface_index, index..step);
    let new_utxos = resolver
        .resolve_utxo(new_scripts)
        .expect("service unavalible");

    for mut new_utxo in new_utxos {
        if let Some(current_utxo) = wallet
            .utxos
            .clone()
            .into_iter()
            .find(|u| u.outpoint == new_utxo.outpoint)
        {
            if current_utxo.status == MiningStatus::Mempool {
                wallet.utxos.remove(&current_utxo);

                new_utxo.derivation = current_utxo.derivation;
                wallet.utxos.insert(new_utxo);
            }
        } else {
            wallet.utxos.insert(new_utxo);
        }
    }
}

pub fn register_address<T>(
    address: &str,
    asset_indexes: Vec<u32>,
    wallet: &mut RgbWallet,
    resolver: &mut T,
    limit: Option<u32>,
) -> Result<Vec<Utxo>, anyhow::Error>
where
    T: ResolveTx + Resolver,
{
    let index = 0;
    let mut step = 100;
    if let Some(limit) = limit {
        step = limit;
    }

    let sc = AddressCompat::from_str(address).expect("invalid address");
    let script = ScriptBuf::from_hex(&sc.script_pubkey().to_hex()).expect("invalid script");

    let mut scripts: BTreeMap<DeriveInfo, ScriptBuf> = BTreeMap::new();
    for app in asset_indexes {
        scripts.append(&mut wallet.descr.derive(app, index..step));
    }
    let script = scripts.into_iter().find(|(_, sc)| sc.eq(&script));
    let mut utxos = vec![];

    if let Some((d, sc)) = script {
        let mut scripts = BTreeMap::new();
        scripts.insert(d.clone(), sc);

        let new_utxos = &resolver.resolve_utxo(scripts).expect("service unavalible");
        for utxo in new_utxos {
            wallet.utxos.insert(utxo.to_owned());
        }

        utxos = new_utxos.iter().map(|u| u.to_owned()).collect();
    }
    Ok(utxos)
}

pub fn register_utxo<T>(
    utxo: &str,
    network: AddressNetwork,
    asset_indexes: Vec<u32>,
    wallet: &mut RgbWallet,
    resolver: &mut T,
    limit: Option<u32>,
) -> Result<Vec<Utxo>, anyhow::Error>
where
    T: ResolveTx + Resolver,
{
    let outpoint = OutPoint::from_str(utxo).expect("invalid outpoint");
    let txid = bp::Txid::from_str(&outpoint.txid.to_hex()).expect("invalid txid");

    let mut utxos = vec![];
    if let Ok(tx) = resolver.resolve_tx(txid) {
        if let Some(vout) = tx.outputs.to_vec().get(outpoint.vout as usize) {
            let sc = Script::from_str(&vout.script_pubkey.to_hex()).expect("invalid script");
            let pub_script = PubkeyScript::from(sc);
            if let Some(address) = AddressCompat::from_script(&pub_script, network) {
                utxos =
                    register_address(&address.to_string(), asset_indexes, wallet, resolver, limit)
                        .expect("invalid utxos");
            }
        }
    }
    Ok(utxos)
}

pub fn list_allocations<T>(
    wallet: &mut RgbWallet,
    stock: &mut Stock,
    iface_index: u32,
    resolver: &mut T,
) -> Result<Vec<WatcherDetail>, anyhow::Error>
where
    T: ResolveSpent + Resolver,
{
    let iface_name = match iface_index {
        20 => "RGB20",
        21 => "RGB21",
        _ => "Contract",
    };

    sync_wallet(iface_index, wallet, resolver);
    let mut details = vec![];
    for contract_id in stock.contract_ids()? {
        let iface = stock.iface_by_name(&tn!(iface_name))?;
        if let Ok(contract) = stock.contract_iface(contract_id, iface.iface_id()) {
            let mut owners = vec![];
            for owned in &contract.iface.assignments {
                if let Ok(allocations) = contract.fungible(owned.name.clone(), &None) {
                    for allocation in allocations {
                        let txid = bitcoin::Txid::from_str(&allocation.owner.txid.to_hex())
                            .expect("invalid txid");
                        let index = allocation.owner.vout.into_u32();
                        let utxo_spent = resolver
                            .resolve_spent_status(txid, index.into(), false)
                            .expect("cannot find utxo");

                        if utxo_spent.clone().is_invalid_state() {
                            continue;
                        }

                        let UtxoSpentStatus { is_spent, .. } = utxo_spent;

                        if let Some(utxo) = wallet.utxo(allocation.owner) {
                            owners.push(AllocationDetail {
                                utxo: utxo.outpoint.to_string(),
                                value: AllocationValue::Value(allocation.value),
                                derivation: format!(
                                    "/{}/{}",
                                    utxo.derivation.terminal.app, utxo.derivation.terminal.index
                                ),
                                is_mine: true,
                                is_spent,
                            });
                        } else {
                            owners.push(AllocationDetail {
                                utxo: allocation.owner.to_string(),
                                value: AllocationValue::Value(allocation.value),
                                derivation: default!(),
                                is_mine: false,
                                is_spent,
                            });
                        }
                    }
                }

                if let Ok(allocations) = contract.data(owned.name.clone()) {
                    for allocation in allocations {
                        let txid = bitcoin::Txid::from_str(&allocation.owner.txid.to_hex())
                            .expect("invalid txid");

                        let index = allocation.owner.vout.into_u32();
                        let utxo_spent = resolver
                            .resolve_spent_status(txid, index.into(), false)
                            .expect("cannot find utxo");

                        if utxo_spent.clone().is_invalid_state() {
                            continue;
                        }

                        let UtxoSpentStatus { is_spent, .. } = utxo_spent;

                        if let Some(utxo) = wallet.utxo(allocation.owner) {
                            owners.push(AllocationDetail {
                                utxo: utxo.outpoint.to_string(),
                                value: AllocationValue::UDA(UDAPosition::with(allocation.value)),
                                derivation: format!(
                                    "/{}/{}",
                                    utxo.derivation.terminal.app, utxo.derivation.terminal.index
                                ),
                                is_mine: true,
                                is_spent,
                            });
                        } else {
                            owners.push(AllocationDetail {
                                utxo: allocation.owner.to_string(),
                                value: AllocationValue::UDA(UDAPosition::with(allocation.value)),
                                derivation: default!(),
                                is_mine: false,
                                is_spent,
                            });
                        }
                    }
                }
            }
            details.push(WatcherDetail {
                contract_id: contract_id.to_string(),
                allocations: owners,
            });
        }
    }

    Ok(details)
}

pub fn contract_allocations<T>(
    contract_id: ContractId,
    iface_index: u32,
    wallet: &mut RgbWallet,
    stock: &mut Stock,
    resolver: &mut T,
) -> Result<WatcherDetail, anyhow::Error>
where
    T: ResolveSpent + Resolver,
{
    let iface_name = match iface_index {
        20 => "RGB20",
        21 => "RGB21",
        _ => "Contract",
    };
    let iface = stock.iface_by_name(&tn!(iface_name))?;

    let mut owners = vec![];
    if let Ok(contract) = stock.contract_iface(contract_id, iface.iface_id()) {
        sync_wallet(iface_index, wallet, resolver);
        for owned in &contract.iface.assignments {
            if let Ok(allocations) = contract.fungible(owned.name.clone(), &None) {
                for allocation in allocations {
                    let txid = bitcoin::Txid::from_str(&allocation.owner.txid.to_hex())
                        .expect("invalid txid");
                    let index = allocation.owner.vout.into_u32();
                    let utxo_spent = resolver
                        .resolve_spent_status(txid, index.into(), true)
                        .expect("cannot find utxo");

                    if utxo_spent.clone().is_invalid_state() {
                        continue;
                    }

                    let UtxoSpentStatus { is_spent, .. } = utxo_spent;
                    if let Some(utxo) = wallet.utxo(allocation.owner) {
                        owners.push(AllocationDetail {
                            utxo: utxo.outpoint.to_string(),
                            value: AllocationValue::Value(allocation.value),
                            derivation: format!(
                                "/{}/{}",
                                utxo.derivation.terminal.app, utxo.derivation.terminal.index
                            ),
                            is_mine: true,
                            is_spent,
                        });
                    } else {
                        owners.push(AllocationDetail {
                            utxo: allocation.owner.to_string(),
                            value: AllocationValue::Value(allocation.value),
                            derivation: default!(),
                            is_mine: false,
                            is_spent,
                        });
                    }
                }
            }

            if let Ok(allocations) = contract.data(owned.name.clone()) {
                for allocation in allocations {
                    let txid = bitcoin::Txid::from_str(&allocation.owner.txid.to_hex())
                        .expect("invalid txid");
                    let index = allocation.owner.vout.into_u32();
                    let utxo_spent = resolver
                        .resolve_spent_status(txid, index.into(), true)
                        .expect("cannot find utxo");

                    if utxo_spent.clone().is_invalid_state() {
                        continue;
                    }

                    let UtxoSpentStatus { is_spent, .. } = utxo_spent;
                    if let Some(utxo) = wallet.utxo(allocation.owner) {
                        owners.push(AllocationDetail {
                            utxo: utxo.outpoint.to_string(),
                            value: AllocationValue::UDA(UDAPosition::with(allocation.value)),
                            derivation: format!(
                                "/{}/{}",
                                utxo.derivation.terminal.app, utxo.derivation.terminal.index
                            ),
                            is_mine: true,
                            is_spent,
                        });
                    } else {
                        owners.push(AllocationDetail {
                            utxo: allocation.owner.to_string(),
                            value: AllocationValue::UDA(UDAPosition::with(allocation.value)),
                            derivation: default!(),
                            is_mine: false,
                            is_spent,
                        });
                    }
                }
            }
        }
    }

    Ok(WatcherDetail {
        contract_id: contract_id.to_string(),
        allocations: owners,
    })
}
