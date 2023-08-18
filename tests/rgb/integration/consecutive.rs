#![cfg(not(target_arch = "wasm32"))]

use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, new_mnemonic, save_mnemonic, sign_psbt_file, sync_wallet},
    rgb::{
        accept_transfer, create_watcher, full_transfer_asset, get_contract, save_transfer,
        verify_transfers,
    },
    structs::{
        AcceptRequest, FullRgbTransferRequest, IssueResponse, PsbtFeeRequest,
        RgbSaveTransferRequest, RgbTransferResponse, SecretString, SignPsbtRequest, WatcherRequest,
    },
};

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_invoice_v2, get_uda_data, issuer_issue_contract_v2,
    send_some_coins, UtxoFilter, ISSUER_MNEMONIC, OWNER_MNEMONIC,
};

#[ignore]
#[tokio::test]
async fn allow_fungible_full_transfer_op() -> anyhow::Result<()> {
    // 1. Initial Setup
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        1,
        false,
        true,
        None,
        Some("0.00000546".to_string()),
        Some(UtxoFilter::with_amount_less_than(546)),
        None,
    )
    .await?;

    // 2. Get Invoice
    let issuer_resp = issuer_resp[0].clone();
    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 3. Get Bitcoin UTXO
    let issuer_btc_desc = &issuer_keys.public.btc_change_descriptor_xpub;
    let issuer_vault = get_wallet(&SecretString(issuer_btc_desc.to_string()), None).await?;
    let issuer_address = &issuer_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(issuer_address, "0.001").await;
    sync_wallet(&issuer_vault).await?;

    // 4. Make a Self Payment
    let self_pay_req = FullRgbTransferRequest {
        contract_id: issuer_resp.contract_id,
        iface: issuer_resp.iface,
        rgb_invoice: owner_resp.invoice.to_string(),
        descriptor: SecretString(issuer_keys.public.rgb_assets_descriptor_xpub.to_string()),
        change_terminal: "/20/1".to_string(),
        fee: PsbtFeeRequest::Value(1000),
        bitcoin_changes: vec![],
    };

    let issue_sk = issuer_keys.private.nostr_prv.to_string();
    let resp = full_transfer_asset(&issue_sk, self_pay_req).await;

    assert!(resp.is_ok());
    Ok(())
}

#[ignore]
#[tokio::test]
async fn allow_uda_full_transfer_op() -> anyhow::Result<()> {
    // 1. Initial Setup
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let meta = Some(get_uda_data());
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB21",
        1,
        false,
        true,
        meta,
        Some("0.00000546".to_string()),
        Some(UtxoFilter::with_amount_less_than(546)),
        None,
    )
    .await?;

    // 2. Get Invoice
    let issuer_resp = issuer_resp[0].clone();
    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys.clone(),
        None,
        None,
    )
    .await?;

    // 3. Get Bitcoin UTXO
    let issuer_btc_desc = &issuer_keys.public.btc_change_descriptor_xpub;
    let issuer_vault = get_wallet(&SecretString(issuer_btc_desc.to_string()), None).await?;
    let issuer_address = &issuer_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(issuer_address, "0.001").await;
    sync_wallet(&issuer_vault).await?;

    // 4. Make a Self Payment
    let self_pay_req = FullRgbTransferRequest {
        contract_id: issuer_resp.contract_id,
        iface: issuer_resp.iface,
        rgb_invoice: owner_resp.invoice.to_string(),
        descriptor: SecretString(issuer_keys.public.rgb_udas_descriptor_xpub.to_string()),
        change_terminal: "/21/1".to_string(),
        fee: PsbtFeeRequest::Value(546),
        bitcoin_changes: vec![],
    };

    let issue_sk = issuer_keys.private.nostr_prv.to_string();
    let resp = full_transfer_asset(&issue_sk, self_pay_req).await;
    assert!(resp.is_ok());

    Ok(())
}

#[ignore]
#[tokio::test]
async fn allow_consecutive_full_transfer_bidirectional() -> anyhow::Result<()> {
    // 1. Initial Setup
    let another_wallet = "bcrt1pps5lhtvuf0la6zq2dgu5h2q2tjdvc7sndkqj5x45qx303p2ln8yslk92wv";
    let wallet_a = new_mnemonic(&SecretString(String::new())).await?;
    let wallet_b = new_mnemonic(&SecretString(String::new())).await?;

    let wallet_a_desc = &wallet_a.public.btc_descriptor_xpub;
    let wallet_b_desc = &wallet_b.public.btc_descriptor_xpub;

    let wallet_a_change_desc = &wallet_a.public.btc_change_descriptor_xpub;
    let wallet_b_change_desc = &wallet_b.public.btc_change_descriptor_xpub;

    let wallet_a_vault = get_wallet(&SecretString(wallet_a_desc.to_string()), None).await?;
    let wallet_b_vault = get_wallet(&SecretString(wallet_b_desc.to_string()), None).await?;

    let wallet_a_address = &wallet_a_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    let wallet_b_address = &wallet_b_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(wallet_a_address, "1").await;
    send_some_coins(wallet_b_address, "1").await;

    let wallet_a_sk = &wallet_a.clone().private.nostr_prv;
    let wallet_b_sk = &wallet_b.clone().private.nostr_prv;

    let wallet_a_watcher = &wallet_a.clone().public.watcher_xpub;
    let create_watcher_a = WatcherRequest {
        name: "default".to_string(),
        xpub: wallet_a_watcher.to_string(),
        force: false,
    };
    let wallet_b_watcher = &wallet_b.clone().public.watcher_xpub;
    let create_watcher_b = WatcherRequest {
        name: "default".to_string(),
        xpub: wallet_b_watcher.to_string(),
        force: false,
    };

    let _ = create_watcher(wallet_a_sk, create_watcher_a).await;
    let _ = create_watcher(wallet_b_sk, create_watcher_b).await;

    // 2. Generate Funded Vault
    let wallet_a_desc = &wallet_a.public.rgb_assets_descriptor_xpub;
    let wallet_b_desc = &wallet_b.public.rgb_assets_descriptor_xpub;

    let wallet_a_vault = get_wallet(
        &SecretString(wallet_a_desc.to_string()),
        Some(&SecretString(wallet_a_change_desc.to_string())),
    )
    .await?;
    let wallet_b_vault = get_wallet(
        &SecretString(wallet_b_desc.to_string()),
        Some(&SecretString(wallet_b_change_desc.to_string())),
    )
    .await?;

    let wallet_a_address = &wallet_a_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    let wallet_b_address = &wallet_b_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(wallet_a_address, "0.00000546").await;
    send_some_coins(wallet_a_address, "0.00000546").await;
    send_some_coins(wallet_b_address, "0.00000546").await;
    send_some_coins(wallet_b_address, "0.00000546").await;
    sync_wallet(&wallet_a_vault).await?;
    sync_wallet(&wallet_b_vault).await?;

    // 3.
    let contract_issue = issuer_issue_contract_v2(
        1,
        "RGB20",
        15_000,
        false,
        false,
        None,
        None,
        Some(UtxoFilter::with_amount_equal_than(546)),
        Some(wallet_a.clone()),
    )
    .await?;
    let contract_issue = contract_issue[0].clone();

    let IssueResponse {
        contract_id,
        iimpl_id: _,
        iface,
        issue_method: _,
        issue_utxo: _,
        ticker: _,
        name: _,
        created: _,
        description: _,
        supply: _,
        precision: _,
        contract,
        genesis: _,
        meta: _,
    } = contract_issue;

    // 4. Make a Self Payment
    for i in 1..10 {
        // Wallet B Invoice
        sync_wallet(&wallet_b_vault).await?;
        let utxo_unspent = wallet_b_vault.lock().await.list_unspent().expect("");
        let utxo_unspent = utxo_unspent.last().unwrap();
        let wallet_b_invoice = create_new_invoice_v2(
            &contract_id.to_string(),
            &iface.to_string(),
            1_000,
            &utxo_unspent.outpoint.to_string(),
            wallet_b.clone(),
            None,
            Some(contract.armored.clone()),
        )
        .await?;

        let self_pay_req = FullRgbTransferRequest {
            contract_id: contract_id.clone(),
            iface: iface.clone(),
            rgb_invoice: wallet_b_invoice.invoice,
            descriptor: SecretString(wallet_a_desc.to_string()),
            change_terminal: "/20/1".to_string(),
            fee: PsbtFeeRequest::Value(546),
            bitcoin_changes: vec![],
        };

        let full_transfer_resp = full_transfer_asset(wallet_a_sk, self_pay_req).await;
        println!("Payment A #{i}:1 ({})", full_transfer_resp.is_ok());

        let full_transfer_resp = full_transfer_resp?;
        let RgbTransferResponse {
            consig_id: _,
            consig,
            psbt,
            commit: _,
        } = full_transfer_resp;

        let request = SignPsbtRequest {
            psbt,
            descriptors: vec![
                SecretString(wallet_a.private.rgb_assets_descriptor_xprv.clone()),
                SecretString(wallet_a.private.btc_descriptor_xprv.clone()),
                SecretString(wallet_a.private.btc_change_descriptor_xprv.clone()),
            ],
        };
        let resp = sign_psbt_file(request).await;
        // println!("{:#?}", resp);
        assert!(resp.is_ok());

        let request = AcceptRequest {
            consignment: consig.clone(),
            force: false,
        };
        let resp = accept_transfer(wallet_a_sk, request.clone()).await;
        assert!(resp.is_ok());
        let request = AcceptRequest {
            consignment: consig.clone(),
            force: false,
        };
        let resp = accept_transfer(wallet_b_sk, request.clone()).await;
        assert!(resp.is_ok());

        send_some_coins(another_wallet, "0.00000546").await;

        for j in 1..2 {
            // Wallet A Invoice
            sync_wallet(&wallet_a_vault).await?;
            let utxo_unspent = wallet_a_vault.lock().await.list_unspent().expect("");
            let utxo_unspent = utxo_unspent.last().unwrap();
            let wallet_a_invoice = create_new_invoice_v2(
                &contract_id.to_string(),
                &iface.to_string(),
                50,
                &utxo_unspent.outpoint.to_string(),
                wallet_a.clone(),
                None,
                Some(contract.armored.clone()),
            )
            .await?;

            let self_pay_req = FullRgbTransferRequest {
                contract_id: contract_id.clone(),
                iface: iface.clone(),
                rgb_invoice: wallet_a_invoice.invoice,
                descriptor: SecretString(wallet_b_desc.to_string()),
                change_terminal: "/20/1".to_string(),
                fee: PsbtFeeRequest::Value(546),
                bitcoin_changes: vec![],
            };

            let full_transfer_resp = full_transfer_asset(wallet_b_sk, self_pay_req).await;
            println!("Payment B #{i}:{j} ({})", full_transfer_resp.is_ok());

            let full_transfer_resp = full_transfer_resp?;
            let RgbTransferResponse {
                consig_id: _,
                consig,
                psbt,
                commit: _,
            } = full_transfer_resp;

            let request = SignPsbtRequest {
                psbt,
                descriptors: vec![
                    SecretString(wallet_b.private.rgb_assets_descriptor_xprv.clone()),
                    SecretString(wallet_b.private.btc_descriptor_xprv.clone()),
                    SecretString(wallet_b.private.btc_change_descriptor_xprv.clone()),
                ],
            };
            let resp = sign_psbt_file(request).await;
            // println!("{:#?}", resp);
            assert!(resp.is_ok());

            let request = AcceptRequest {
                consignment: consig.clone(),
                force: false,
            };
            let resp = accept_transfer(wallet_a_sk, request.clone()).await;
            assert!(resp.is_ok());
            let request = AcceptRequest {
                consignment: consig.clone(),
                force: false,
            };
            let resp = accept_transfer(wallet_b_sk, request.clone()).await;
            assert!(resp.is_ok());
        }

        send_some_coins(another_wallet, "0.00000546").await;
    }

    let _contract_a = get_contract(wallet_a_sk, &contract_id).await?;
    let _contract_b = get_contract(wallet_b_sk, &contract_id).await?;

    // println!(
    //     "Contract A: {}\nAllocations: {:#?}\n\n",
    //     contract_a.contract_id, contract_a.allocations
    // );
    // println!(
    //     "Contract B: {}\nAllocations: {:#?}",
    //     contract_b.contract_id, contract_b.allocations
    // );

    Ok(())
}

#[tokio::test]
async fn allow_save_transfer_and_verify() -> anyhow::Result<()> {
    // 1. Initial Setup
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        1,
        false,
        true,
        None,
        Some("0.00000546".to_string()),
        Some(UtxoFilter::with_amount_less_than(546)),
        None,
    )
    .await?;

    let issuer_watcher_key = &issuer_keys.public.watcher_xpub;
    let issuer_watcher = WatcherRequest {
        name: "default".to_string(),
        xpub: issuer_watcher_key.to_string(),
        force: false,
    };
    let issue_sk = issuer_keys.private.nostr_prv.to_string();
    create_watcher(&issue_sk, issuer_watcher).await?;

    let owner_watcher_key = &owner_keys.public.watcher_xpub;
    let owner_watcher = WatcherRequest {
        name: "default".to_string(),
        xpub: owner_watcher_key.to_string(),
        force: false,
    };
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    create_watcher(&owner_sk, owner_watcher).await?;

    // 2. Get Invoice
    let issuer_resp = issuer_resp[0].clone();

    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 3. Get Bitcoin UTXO
    let issuer_btc_desc = &issuer_keys.public.btc_change_descriptor_xpub;
    let issuer_vault = get_wallet(&SecretString(issuer_btc_desc.to_string()), None).await?;
    let issuer_address = &issuer_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(issuer_address, "0.001").await;
    sync_wallet(&issuer_vault).await?;

    // 4. Make a Self Payment
    let self_pay_req = FullRgbTransferRequest {
        contract_id: issuer_resp.contract_id.clone(),
        iface: issuer_resp.iface.clone(),
        rgb_invoice: owner_resp.invoice.to_string(),
        descriptor: SecretString(issuer_keys.public.rgb_assets_descriptor_xpub.to_string()),
        change_terminal: "/20/1".to_string(),
        fee: PsbtFeeRequest::Value(1000),
        bitcoin_changes: vec![],
    };

    let resp = full_transfer_asset(&issue_sk, self_pay_req).await?;

    let RgbTransferResponse {
        consig_id: _,
        consig,
        psbt,
        commit: _,
    } = resp;

    let request = SignPsbtRequest {
        psbt,
        descriptors: vec![
            SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
            SecretString(issuer_keys.private.btc_descriptor_xprv.clone()),
            SecretString(issuer_keys.private.btc_change_descriptor_xprv.clone()),
        ],
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    let owner_sk = owner_keys.private.nostr_prv.clone();
    let request = RgbSaveTransferRequest {
        iface: issuer_resp.iface.clone(),
        contract_id: issuer_resp.contract_id.clone(),
        consignment: consig,
    };
    let resp = save_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());

    let resp = verify_transfers(&owner_sk).await;
    assert!(resp.is_ok());

    let contract = get_contract(&owner_sk, &issuer_resp.contract_id).await?;
    assert_eq!(contract.balance, 1);

    Ok(())
}
