#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bdk::{
    database::MemoryDatabase,
    descriptor::IntoWalletDescriptor,
    wallet::{tx_builder::TxOrdering, AddressIndex},
    SignOptions, SyncOptions,
};
use bitcoin::{secp256k1::Secp256k1, Network};
use bitmask_core::{
    bitcoin::{get_blockchain, new_mnemonic},
    structs::SecretString,
};

use crate::rgb::integration::utils::send_some_coins;

#[ignore = "No longer necessary, this is a simple test to rbf with bdk"]
#[tokio::test]
pub async fn create_bdk_rbf_transaction() -> Result<()> {
    // 1. Initial Setup
    let user_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let blockchain = get_blockchain().await;
    let secp = Secp256k1::new();
    let db = MemoryDatabase::new();
    let descriptor = user_keys
        .private
        .btc_descriptor_xprv
        .into_wallet_descriptor(&secp, Network::Regtest)?;
    let user_wallet_data = bdk::Wallet::new(descriptor, None, Network::Regtest, db)?;

    let user_address = user_wallet_data.get_address(AddressIndex::New)?;
    send_some_coins(&user_address.address.to_string(), "1").await;

    user_wallet_data
        .sync(&blockchain, SyncOptions::default())
        .await?;
    let mut builder = user_wallet_data.build_tx();

    let address = user_wallet_data.get_address(AddressIndex::New)?;
    builder.add_recipient(address.address.script_pubkey(), 100000);

    builder.ordering(TxOrdering::Bip69Lexicographic);
    builder.fee_rate(bdk::FeeRate::from_sat_per_vb(1.0));
    builder.enable_rbf();

    let (mut psbt, _) = builder.finish()?;

    let _ = user_wallet_data.sign(&mut psbt, SignOptions::default())?;
    // println!("{:#?}", signed);

    let tx = psbt.extract_tx();

    blockchain.broadcast(&tx).await?;

    user_wallet_data
        .sync(&blockchain, SyncOptions::default())
        .await?;

    let txs = user_wallet_data.list_transactions(false)?;
    // println!("{:#?}", txs);

    assert_eq!(2, txs.len());

    let tx_1_utxos: Vec<String> = tx
        .input
        .clone()
        .into_iter()
        .map(|u| u.previous_output.to_string())
        .collect();

    let (mut psbt, ..) = {
        let mut builder = user_wallet_data.build_fee_bump(tx.txid())?;
        builder.fee_rate(bdk::FeeRate::from_sat_per_vb(5.0));
        builder.finish()?
    };

    let _ = user_wallet_data.sign(&mut psbt, SignOptions::default())?;
    let tx = psbt.extract_tx();
    let blockchain = get_blockchain().await;
    blockchain.broadcast(&tx).await?;

    user_wallet_data
        .sync(&blockchain, SyncOptions::default())
        .await?;

    let txs = user_wallet_data.list_transactions(false)?;
    // println!("{:#?}", txs);

    assert_eq!(2, txs.len());

    let tx_2_utxos: Vec<String> = tx
        .input
        .into_iter()
        .map(|u| u.previous_output.to_string())
        .collect();

    // println!("{:#?}", tx_1_utxos);
    // println!("{:#?}", tx_2_utxos);
    assert_eq!(tx_1_utxos, tx_2_utxos);

    Ok(())
}
