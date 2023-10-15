#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::get_raw_wallet;
use amplify::confinement::Collection;
use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use bitmask_core::rgb::crdt::{RawRgbWallet, RawUtxo};
use rgb::{RgbWallet, Utxo};

#[tokio::test]
async fn allow_fork_with_previous_version() -> anyhow::Result<()> {
    let current_raw_wallet = get_raw_wallet();
    let new_raw_utxo = RawUtxo {
        outpoint: "9a5d21d4cc15ffa14c6f416396235c082cddb5e227abd863974445709f8e9af0:0".to_string(),
        block: 0,
        amount: 10000000,
        terminal: "20:1".to_string(),
        tweak: Some("gu0x5*b6~L+)(kY3<hZqSKwHx1{ezy#_l#3uaPPG00".to_string()),
    };

    let new_utxo = Utxo::from(new_raw_utxo);
    let mut rgb_wallet = RgbWallet::from(current_raw_wallet.clone());

    let mut original = AutoCommit::new();
    reconcile(&mut original, current_raw_wallet)?;

    let mut fork = original.fork();

    rgb_wallet.utxos.push(new_utxo.clone());
    let latest = RawRgbWallet::from(rgb_wallet);
    reconcile(&mut fork, latest)?;

    original.merge(&mut fork)?;

    let merged: RawRgbWallet = hydrate(&original).unwrap();
    let merged = RgbWallet::from(merged);

    assert!(merged.utxo(new_utxo.outpoint).is_some());

    Ok(())
}

#[tokio::test]
async fn allow_fork_without_previous_version() -> anyhow::Result<()> {
    let current_raw_wallet = get_raw_wallet();
    let new_raw_utxo = RawUtxo {
        outpoint: "9a5d21d4cc15ffa14c6f416396235c082cddb5e227abd863974445709f8e9af0:0".to_string(),
        block: 0,
        amount: 10000000,
        terminal: "20:1".to_string(),
        tweak: Some("gu0x5*b6~L+)(kY3<hZqSKwHx1{ezy#_l#3uaPPG00".to_string()),
    };

    let new_utxo = Utxo::from(new_raw_utxo);
    let mut rgb_wallet = RgbWallet::from(current_raw_wallet.clone());

    // Create Original
    let mut original = AutoCommit::new();
    reconcile(&mut original, current_raw_wallet.clone())?;

    // Rebase Fork
    let mut fork = original.fork();

    rgb_wallet.utxos.push(new_utxo.clone());
    let latest = RawRgbWallet::from(rgb_wallet);
    reconcile(&mut fork, latest)?;

    // Rebase Original
    let mut original = AutoCommit::new();
    original.merge(&mut fork)?;

    let merged: RawRgbWallet = hydrate(&original).unwrap();
    let merged = RgbWallet::from(merged);

    assert!(merged.utxo(new_utxo.outpoint).is_some());

    Ok(())
}
