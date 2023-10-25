#![cfg(not(target_arch = "wasm32"))]

use std::str::FromStr;

use anyhow::Result;
use bitcoin::Txid;
use bitmask_core::{
    bitcoin::{get_blockchain, new_mnemonic, sign_and_publish_psbt_file},
    rgb::{
        accept_transfer, consignment::NewTransferOptions, create_watcher, get_contract,
        internal_replace_transfer, issue_contract, list_contracts, structs::ContractAmount,
        watcher_next_address,
    },
    structs::{
        AcceptRequest, IssueRequest, PsbtFeeRequest, PublishedPsbtResponse, RgbReplaceResponse,
        RgbTransferRequest, RgbTransferResponse, SecretString, SignPsbtRequest, WatcherRequest,
    },
};
use rgbwallet::RgbInvoice;

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt_v2, create_new_transfer, generate_new_block, get_uda_data,
    issuer_issue_contract_v2, send_some_coins, UtxoFilter,
};

#[tokio::test]
pub async fn create_strict_transfer() -> Result<()> {
    // 1. Initial Setup
    let _whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let owner_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let issuer_sk = &issuer_keys.private.nostr_prv;
    let fungibles_resp = issuer_issue_contract_v2(
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
    let issuer_resp = &fungibles_resp[0];

    let spec = "UDA".to_string();
    let meta = Some(get_uda_data());
    let issue_utxo = issuer_resp.issue_utxo.clone();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let issue_uda_req = IssueRequest {
        ticker: spec.clone(),
        name: spec.clone(),
        description: spec.clone(),
        precision: 0,
        supply: 1,
        seal: issue_seal.to_owned(),
        iface: "RGB21".to_string(),
        meta,
    };

    let _uda_resp = issue_contract(issuer_sk, issue_uda_req).await?;
    generate_new_block().await;

    // 2. Get Allocations
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
        vec![],
        None,
    )
    .await?;

    // 3. Generate Invoice
    let watcher_name = "default";
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&owner_sk, create_watch_req).await?;
    let owner_fungible_address = watcher_next_address(&owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&owner_fungible_address.address, "1").await;

    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1.0,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 4. Generate Transfer
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    let RgbTransferResponse { psbt, .. } = transfer_resp;
    let psbt_req = SignPsbtRequest {
        psbt: psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    // 5. Accept Transfer
    generate_new_block().await;
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: transfer_resp.consig.clone(),
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
    }

    // 6. Check Facts
    let issuer_contracts = list_contracts(issuer_sk, false).await?;
    let owner_contracts = list_contracts(&owner_sk, false).await?;

    assert_eq!(issuer_contracts.contracts.len(), 2);
    assert_eq!(owner_contracts.contracts.len(), 1);
    Ok(())
}

#[tokio::test]
pub async fn create_transfer_rbf() -> Result<()> {
    // 1. Initial Setup
    let _whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let owner_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let issuer_sk = &issuer_keys.private.nostr_prv;
    let fungibles_resp = issuer_issue_contract_v2(
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
    let issuer_resp = &fungibles_resp[0];

    let spec = "UDA".to_string();
    let meta = Some(get_uda_data());
    let issue_utxo = issuer_resp.issue_utxo.clone();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let issue_uda_req = IssueRequest {
        ticker: spec.clone(),
        name: spec.clone(),
        description: spec.clone(),
        precision: 0,
        supply: 1,
        seal: issue_seal.to_owned(),
        iface: "RGB21".to_string(),
        meta,
    };

    let _uda_resp = issue_contract(issuer_sk, issue_uda_req).await?;
    generate_new_block().await;

    // 2. Get Allocations
    let contract_id = &issuer_resp.contract_id;
    let issuer_contract = get_contract(issuer_sk, contract_id).await?;
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine)
        .unwrap();
    let allocs = [new_alloc];

    // 3. Create PSBT (First Transaction)
    let psbt_resp = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        vec![],
        vec![],
        None,
    )
    .await?;

    // 4. Generate Invoice
    let watcher_name = "default";
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&owner_sk, create_watch_req).await?;
    let owner_fungible_address = watcher_next_address(&owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&owner_fungible_address.address, "1").await;

    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1.0,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 5. Generate Transfer
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    let RgbTransferResponse { psbt, .. } = transfer_resp;
    let psbt_req = SignPsbtRequest {
        psbt: psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    // 6. Check Mempool transaction
    let txid1 = Txid::from_str(&psbt_resp?.txid)?;
    let explorer = get_blockchain().await;
    let transaction = explorer.get_tx(&txid1).await;
    assert!(transaction.is_ok());
    assert!(transaction?.is_some());

    // 7. Create PSBT (Second Transaction)
    let psbt_resp = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        vec![],
        vec![],
        Some(PsbtFeeRequest::Value(2000)),
    )
    .await?;

    // 8. Generate Second (Second Transaction)
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    // 9. Sign and Broadcast
    let RgbTransferResponse { psbt, .. } = transfer_resp;
    let psbt_req = SignPsbtRequest {
        psbt: psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };

    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    // 10. Accept Transfer
    generate_new_block().await;
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: transfer_resp.consig.clone(),
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
    }

    // 11. Check Facts
    let issuer_contracts = list_contracts(issuer_sk, false).await?;
    let owner_contracts = list_contracts(&owner_sk, false).await?;

    assert_eq!(issuer_contracts.contracts.len(), 2);
    assert_eq!(owner_contracts.contracts.len(), 1);
    Ok(())
}

#[tokio::test]
pub async fn create_batch_transfer() -> Result<()> {
    // 1. Initial Setup
    let watcher_name = "default";
    let issuer_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let owner_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let other_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let issuer_sk = &issuer_keys.private.nostr_prv;
    let fungibles_resp = issuer_issue_contract_v2(
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
    let issuer_resp = &fungibles_resp[0];

    let spec = "UDA".to_string();
    let meta = Some(get_uda_data());
    let issue_utxo = issuer_resp.issue_utxo.clone();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let issue_uda_req = IssueRequest {
        ticker: spec.clone(),
        name: spec.clone(),
        description: spec.clone(),
        precision: 0,
        supply: 1,
        seal: issue_seal.to_owned(),
        iface: "RGB21".to_string(),
        meta,
    };

    let _uda_resp = issue_contract(issuer_sk, issue_uda_req).await?;
    generate_new_block().await;

    // 2. Get Allocations
    let contract_id = &issuer_resp.contract_id;
    let issuer_contract = get_contract(issuer_sk, contract_id).await?;
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine)
        .unwrap();
    let allocs = [new_alloc];

    // 3. Create PSBT (First Transaction)
    let psbt_resp = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        vec![],
        vec![],
        None,
    )
    .await?;

    // 4. Generate Invoice
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&owner_sk, create_watch_req).await?;
    let owner_fungible_address = watcher_next_address(&owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&owner_fungible_address.address, "1").await;

    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1.0,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 5. Generate Another Invoice
    let other_sk = other_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: other_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&other_sk, create_watch_req).await?;

    let other_fungible_address = watcher_next_address(&other_sk, watcher_name, "RGB20").await?;
    send_some_coins(&other_fungible_address.address, "1").await;

    let other_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2.0,
        other_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 6. Generate Transfer (First Owner)
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    let RgbTransferResponse { psbt, .. } = transfer_resp;
    let psbt_req = SignPsbtRequest {
        psbt: psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    let PublishedPsbtResponse { txid, .. } = psbt_resp?;
    // println!("First txid ({sign}) {txid}");

    // 7. Check Mempool transaction
    let txid1 = Txid::from_str(&txid)?;
    let explorer = get_blockchain().await;
    let transaction = explorer.get_tx(&txid1).await;
    assert!(transaction.is_ok());
    assert!(transaction?.is_some());

    // 8. Create PSBT (Second Transaction)
    let psbt_resp_2 = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        vec![],
        vec![],
        Some(PsbtFeeRequest::Value(2000)),
    )
    .await?;

    // 10. Generate Second (Second Transaction)
    let transfer_req = RgbTransferRequest {
        psbt: psbt_resp_2.psbt,
        rgb_invoice: other_resp.invoice.clone(),
        terminal: psbt_resp_2.terminal,
    };

    let rgb_invoice = RgbInvoice::from_str(&owner_resp.invoice)?;
    let options = NewTransferOptions::with(true, vec![rgb_invoice]);

    let replace_transfer_rep = internal_replace_transfer(issuer_sk, transfer_req, options).await?;

    // 9. Sign and Broadcast
    let RgbReplaceResponse {
        psbt,
        consig,
        consigs,
        ..
    } = replace_transfer_rep;
    let psbt_req = SignPsbtRequest {
        psbt: psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };

    let psbt_resp = sign_and_publish_psbt_file(psbt_req).await;
    assert!(psbt_resp.is_ok());

    let PublishedPsbtResponse { .. } = psbt_resp?;
    // println!("Second txid ({sign}) {txid}");

    // 10. Accept Transfer
    generate_new_block().await;
    let all_sks = [issuer_sk.clone(), other_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: consig.clone(),
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
    }

    let prev_consig: Vec<String> = consigs.into_values().collect();
    let prev_consig = &prev_consig[0];
    let request = AcceptRequest {
        consignment: prev_consig.clone(),
        force: false,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());

    // 11. Check Facts
    let issuer_contracts = list_contracts(issuer_sk, false).await?;
    let owner_contracts = list_contracts(&owner_sk, false).await?;
    let other_contracts = list_contracts(&other_sk, false).await?;

    assert_eq!(issuer_contracts.contracts.len(), 2);
    assert_eq!(owner_contracts.contracts.len(), 1);
    assert_eq!(other_contracts.contracts.len(), 1);

    let issuer_contracts = issuer_contracts
        .contracts
        .into_iter()
        .find(|x| x.contract_id == *contract_id)
        .unwrap();
    let owner_contracts = owner_contracts
        .contracts
        .into_iter()
        .find(|x| x.contract_id == *contract_id)
        .unwrap();
    let other_contracts = other_contracts
        .contracts
        .into_iter()
        .find(|x| x.contract_id == *contract_id)
        .unwrap();

    // println!("Issuer Contract: \n {:#?}", issuer_contracts.allocations);
    // println!("Owner Contract: \n {:#?}", owner_contracts.allocations);
    // println!("other Contract: \n {:#?}", other_contracts.allocations);

    assert_eq!(2.0, issuer_contracts.balance_normalised);
    assert_eq!(1.0, owner_contracts.balance_normalised);
    assert_eq!(2.0, other_contracts.balance_normalised);

    Ok(())
}
