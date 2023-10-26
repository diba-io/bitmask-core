#![cfg(not(target_arch = "wasm32"))]
use std::str::FromStr;

use anyhow::Result;
use bdk::{
    database::MemoryDatabase,
    descriptor::IntoWalletDescriptor,
    wallet::{tx_builder::TxOrdering, AddressIndex},
    SignOptions, SyncOptions,
};
use bitcoin::{secp256k1::Secp256k1, Network, Txid};
use bitmask_core::{
    bitcoin::{get_blockchain, new_mnemonic, sign_and_publish_psbt_file},
    rgb::{get_contract, structs::ContractAmount},
    structs::{PsbtFeeRequest, PsbtResponse, SecretString, SignPsbtRequest},
};

use crate::rgb::integration::utils::{
    create_new_psbt_v2, issuer_issue_contract_v2, send_some_coins, UtxoFilter,
};

#[tokio::test]
pub async fn create_simple_rbf_bitcoin_transfer() -> Result<()> {
    // 1. Initial Setup
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        ContractAmount::new(5, 2).to_value(),
        false,
        true,
        None,
        Some("0.10000000".to_string()),
        Some(UtxoFilter::with_amount_equal_than(10_000_000)),
        Some(issuer_keys.clone()),
    )
    .await?;
    let issuer_resp = &issuer_resp[0];
    send_some_coins(whatever_address, "0.1").await;

    // 2. Get Allocations
    let issuer_sk = &issuer_keys.private.nostr_prv;
    let contract_id = &issuer_resp.contract_id;
    let issuer_contract = get_contract(issuer_sk, contract_id).await?;
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine)
        .unwrap();
    let allocs = [new_alloc];

    // 2. Create PSBT (First Transaction)
    let psbt_resp = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        vec![],
        vec![format!("{whatever_address}:1000")],
        None,
    )
    .await?;

    // 3. Sign and Broadcast
    let PsbtResponse { psbt, .. } = psbt_resp;
    let psbt_req = SignPsbtRequest {
        psbt,
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    // 4. Check TX
    let txid1 = Txid::from_str(&psbt_resp?.txid)?;
    let explorer = get_blockchain().await;
    let transaction = explorer.get_tx(&txid1).await;
    assert!(transaction.is_ok());
    assert!(transaction?.is_some());

    // 5. Create PSBT (Second Transaction)
    let psbt_resp = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        vec![],
        vec![format!("{whatever_address}:1000")],
        Some(PsbtFeeRequest::Value(2000)),
    )
    .await?;

    // 6. Sign and Broadcast
    let PsbtResponse { psbt, .. } = psbt_resp;
    let psbt_req = SignPsbtRequest {
        psbt,
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    // 7. Check Both TX
    let txid2 = Txid::from_str(&psbt_resp?.txid)?;
    let explorer = get_blockchain().await;
    let transaction2 = explorer.get_tx(&txid2).await;
    assert!(transaction2.is_ok());

    let transaction1 = explorer.get_tx(&txid1).await;
    assert!(transaction1.is_ok());

    let tx_1 = transaction1?.unwrap();
    let tx_2 = transaction2?.unwrap();

    // 8. Get Wallet
    let secp = Secp256k1::new();
    let db = MemoryDatabase::new();
    let descriptor = issuer_keys
        .private
        .rgb_assets_descriptor_xprv
        .into_wallet_descriptor(&secp, Network::Regtest)?;
    let issuer_vault = bdk::Wallet::new(descriptor, None, Network::Regtest, db)?;
    issuer_vault.sync(&explorer, SyncOptions::default()).await?;

    let list_transactions = &issuer_vault.list_transactions(false)?;
    assert!(!list_transactions.iter().any(|x| x.txid == tx_1.txid()));
    assert!(list_transactions.iter().any(|x| x.txid == tx_2.txid()));

    Ok(())
}

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
