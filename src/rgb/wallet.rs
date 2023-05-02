use amplify::hex::ToHex;
use bitcoin::{Script, Txid};
use bitcoin_30::bip32::ExtendedPubKey;
use bitcoin_hashes::hex::FromHex;
use bitcoin_scripts::address::{AddressCompat, AddressNetwork};
use bp::dbc::tapret::TapretCommitment;
use commit_verify::mpc::Commitment;
use rgb::{DeriveInfo, Resolver, RgbDescr, RgbWallet, SpkDescriptor, Tapret, TerminalPath, Utxo};
use std::{collections::HashMap, str::FromStr};

use super::{resolvers::ResolveSpent, structs::AddressTerminal};

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
            let sc = Script::from_hex(&sb.to_hex()).expect("");
            let address = AddressCompat::from_script(&sc.into(), network).expect("");
            let terminal = d.terminal;
            AddressTerminal { address, terminal }
        })
        .collect())
}

pub fn list_utxos(wallet: RgbWallet) -> Result<Vec<Utxo>, anyhow::Error> {
    Ok(wallet.utxos.into_iter().collect())
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

    let next_index = max + 1;
    let scripts = wallet.descr.derive(iface_index, max..next_index);
    let addresses: Vec<AddressTerminal> = scripts
        .into_iter()
        .map(|(d, sb)| {
            let sc = Script::from_hex(&sb.to_hex()).expect("");
            let address = AddressCompat::from_script(&sc.into(), network).expect("");
            let terminal = d.terminal;
            AddressTerminal { address, terminal }
        })
        .collect();

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
        .filter(|utxo| {
            utxo.derivation.terminal.app == iface_index && utxo.derivation.tweak.is_none()
        })
        .collect();

    if utxos.is_empty() {
        return Ok(none!());
    }

    let mut next_utxo: Option<Utxo> = None;
    for utxo in utxos {
        let txid = Txid::from_hex(&utxo.outpoint.txid.to_hex()).expect("");
        let is_spent = resolver
            .resolve_spent_status(txid, utxo.outpoint.vout.into_u32().into())
            .expect("unavaliable service");
        if !is_spent {
            next_utxo = Some(utxo);
            break;
        }
    }

    Ok(next_utxo)
}

pub fn save_commitment(
    iface_index: u32,
    path: TerminalPath,
    commit: String,
    wallet: &mut RgbWallet,
) {
    let mpc = Commitment::from_str(&commit).expect("invalid commitment");
    let tap_commit = TapretCommitment::with(mpc, 0);

    let mut utxo = wallet
        .utxos
        .clone()
        .into_iter()
        .find(|utxo| {
            utxo.derivation.terminal.app == iface_index && utxo.derivation.terminal == path
        })
        .expect("");

    wallet.utxos.remove(&utxo);
    utxo.derivation.tweak = Some(tap_commit);
    wallet.utxos.insert(utxo);
}
pub fn sync_wallet(iface_index: u32, wallet: &mut RgbWallet, resolver: &mut impl Resolver) {
    let step = 20;
    let mut index = 0;

    let iface_indexes: Vec<u32> = wallet
        .utxos
        .clone()
        .into_iter()
        .filter(|utxo| utxo.derivation.terminal.app == iface_index)
        .map(|utxo| utxo.derivation.terminal.index)
        .collect();

    loop {
        let scripts = wallet.descr.derive(iface_index, index..step);
        let new_scripts = scripts
            .into_iter()
            .filter(|(d, _)| !iface_indexes.contains(&d.terminal.index))
            .map(|(d, sc)| (d, sc))
            .collect();

        let mut new_utxos = resolver.resolve_utxo(new_scripts).expect("");
        if new_utxos.is_empty() {
            break;
        }
        wallet.utxos.append(&mut new_utxos);
        index += step;
    }
}
