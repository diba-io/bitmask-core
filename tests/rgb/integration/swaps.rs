#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    get_uda_data, issuer_issue_contract_v2, send_some_coins, UtxoFilter,
};
use bitmask_core::{
    bitcoin::{
        fund_vault, get_new_address, get_wallet, new_mnemonic, publish_psbt_file,
        sign_and_publish_psbt_file, sign_psbt_file, sync_wallet,
    },
    rgb::{
        accept_transfer, create_buyer_bid, create_seller_offer, create_swap_transfer,
        create_watcher, get_contract, update_seller_offer, verify_transfers,
    },
    structs::{
        AcceptRequest, IssueResponse, PsbtFeeRequest, PublishPsbtRequest, RgbBidRequest,
        RgbBidResponse, RgbOfferRequest, RgbOfferResponse, RgbOfferUpdateRequest, RgbSwapRequest,
        RgbSwapResponse, SecretString, SignPsbtRequest, SignedPsbtResponse, WatcherRequest,
    },
};

#[tokio::test]
async fn create_scriptless_swap() -> anyhow::Result<()> {
    // 1. Initial Setup
    let seller_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let buyer_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let watcher_name = "default";
    let issuer_sk = &seller_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: seller_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = &buyer_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: buyer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(owner_sk, create_watch_req.clone()).await?;

    // 2. Setup Wallets (Seller)
    let btc_address_1 = get_new_address(
        &SecretString(seller_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let default_coins = "0.001";
    send_some_coins(&btc_address_1, default_coins).await;

    let btc_descriptor_xprv = SecretString(seller_keys.private.btc_descriptor_xprv.clone());
    let btc_change_descriptor_xprv =
        SecretString(seller_keys.private.btc_change_descriptor_xprv.clone());

    let assets_address_1 = get_new_address(
        &SecretString(seller_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let assets_address_2 = get_new_address(
        &SecretString(seller_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_1 = get_new_address(
        &SecretString(seller_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_2 = get_new_address(
        &SecretString(seller_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let btc_wallet = get_wallet(&btc_descriptor_xprv, Some(&btc_change_descriptor_xprv)).await?;
    sync_wallet(&btc_wallet).await?;

    let fund_vault = fund_vault(
        &btc_descriptor_xprv,
        &btc_change_descriptor_xprv,
        &assets_address_1,
        &assets_address_2,
        &uda_address_1,
        &uda_address_2,
        Some(1.1),
    )
    .await?;

    // 3. Send some coins (Buyer)
    let btc_address_1 = get_new_address(
        &SecretString(buyer_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;
    let asset_address_1 = get_new_address(
        &SecretString(buyer_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let default_coins = "0.1";
    send_some_coins(&btc_address_1, default_coins).await;
    send_some_coins(&asset_address_1, default_coins).await;

    // 4. Issue Contract (Seller)
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        5,
        false,
        false,
        None,
        None,
        Some(UtxoFilter::with_outpoint(
            fund_vault.assets_output.unwrap_or_default(),
        )),
        Some(seller_keys.clone()),
    )
    .await?;
    let IssueResponse {
        contract_id,
        iface,
        supply,
        ..
    } = issuer_resp[0].clone();

    // 5. Create Seller Swap Side
    let contract_amount = supply - 1;
    let seller_sk = seller_keys.private.nostr_prv.clone();
    let bitcoin_price: u64 = 100000;
    let seller_asset_desc = seller_keys.public.rgb_assets_descriptor_xpub.clone();
    let expire_at = (chrono::Local::now() + chrono::Duration::minutes(5))
        .naive_utc()
        .timestamp();
    let seller_swap_req = RgbOfferRequest {
        contract_id: contract_id.clone(),
        iface,
        contract_amount,
        bitcoin_price,
        descriptor: SecretString(seller_asset_desc),
        change_terminal: "/20/1".to_string(),
        bitcoin_changes: vec![],
        expire_at: Some(expire_at),
    };

    let seller_swap_resp = create_seller_offer(&seller_sk, seller_swap_req).await;
    assert!(seller_swap_resp.is_ok());

    // 7. Create Buyer Swap Side
    let RgbOfferResponse {
        offer_id,
        contract_amount,
        ..
    } = seller_swap_resp?;

    let buyer_sk = buyer_keys.private.nostr_prv.clone();
    let buyer_btc_desc = buyer_keys.public.btc_descriptor_xpub.clone();
    let buyer_swap_req = RgbBidRequest {
        offer_id: offer_id.clone(),
        asset_amount: contract_amount,
        descriptor: SecretString(buyer_btc_desc),
        change_terminal: "/1/0".to_string(),
        fee: PsbtFeeRequest::Value(1000),
    };

    let buyer_swap_resp = create_buyer_bid(&buyer_sk, buyer_swap_req).await;
    assert!(buyer_swap_resp.is_ok());

    // 8. Sign the Buyer Side
    let RgbBidResponse {
        bid_id, swap_psbt, ..
    } = buyer_swap_resp?;
    let request = SignPsbtRequest {
        psbt: swap_psbt,
        descriptors: vec![
            SecretString(buyer_keys.private.btc_descriptor_xprv.clone()),
            SecretString(buyer_keys.private.btc_change_descriptor_xprv.clone()),
        ],
    };
    let buyer_psbt_resp = sign_psbt_file(request).await;
    assert!(buyer_psbt_resp.is_ok());

    // 9. Create Swap PSBT
    let SignedPsbtResponse {
        psbt: swap_psbt, ..
    } = buyer_psbt_resp?;
    let final_swap_req = RgbSwapRequest {
        offer_id,
        bid_id,
        swap_psbt,
    };

    let final_swap_resp = create_swap_transfer(issuer_sk, final_swap_req).await;
    assert!(final_swap_resp.is_ok());

    // 8. Sign the Final PSBT
    let RgbSwapResponse {
        final_consig,
        final_psbt,
        ..
    } = final_swap_resp?;

    let request = SignPsbtRequest {
        psbt: final_psbt.clone(),
        descriptors: vec![
            SecretString(seller_keys.private.btc_descriptor_xprv.clone()),
            SecretString(seller_keys.private.btc_change_descriptor_xprv.clone()),
            SecretString(seller_keys.private.rgb_assets_descriptor_xprv.clone()),
        ],
    };
    let seller_psbt_resp = sign_and_publish_psbt_file(request).await;
    assert!(seller_psbt_resp.is_ok());

    // 9. Accept Consig (Buyer/Seller)
    let all_sks = [buyer_sk.clone(), seller_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: final_consig.clone(),
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 10 Mine Some Blocks
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    send_some_coins(whatever_address, "0.001").await;

    // 11. Retrieve Contract (Seller Side)
    let resp = get_contract(&seller_sk, &contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    // 12. Retrieve Contract (Buyer Side)
    let resp = get_contract(&buyer_sk, &contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(4, resp?.balance);

    // 13. Verify transfers (Seller Side)
    let resp = verify_transfers(&seller_sk).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.transfers.len());

    Ok(())
}

#[tokio::test]
async fn create_scriptless_swap_for_uda() -> anyhow::Result<()> {
    // 1. Initial Setup
    let seller_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let buyer_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let watcher_name = "default";
    let issuer_sk = &seller_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: seller_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = &buyer_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: buyer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(owner_sk, create_watch_req.clone()).await?;

    // 2. Setup Wallets (Seller)
    let btc_address_1 = get_new_address(
        &SecretString(seller_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let default_coins = "0.001";
    send_some_coins(&btc_address_1, default_coins).await;

    let btc_descriptor_xprv = SecretString(seller_keys.private.btc_descriptor_xprv.clone());
    let btc_change_descriptor_xprv =
        SecretString(seller_keys.private.btc_change_descriptor_xprv.clone());

    let assets_address_1 = get_new_address(
        &SecretString(seller_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let assets_address_2 = get_new_address(
        &SecretString(seller_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_1 = get_new_address(
        &SecretString(seller_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_2 = get_new_address(
        &SecretString(seller_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let btc_wallet = get_wallet(&btc_descriptor_xprv, Some(&btc_change_descriptor_xprv)).await?;
    sync_wallet(&btc_wallet).await?;

    let fund_vault = fund_vault(
        &btc_descriptor_xprv,
        &btc_change_descriptor_xprv,
        &assets_address_1,
        &assets_address_2,
        &uda_address_1,
        &uda_address_2,
        Some(1.1),
    )
    .await?;

    // 3. Send some coins (Buyer)
    let btc_address_1 = get_new_address(
        &SecretString(buyer_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;
    let asset_address_1 = get_new_address(
        &SecretString(buyer_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let default_coins = "0.1";
    send_some_coins(&btc_address_1, default_coins).await;
    send_some_coins(&asset_address_1, default_coins).await;

    // 4. Issue Contract (Seller)
    let metadata = get_uda_data();
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB21",
        1,
        false,
        false,
        Some(metadata),
        None,
        Some(UtxoFilter::with_outpoint(
            fund_vault.udas_output.unwrap_or_default(),
        )),
        Some(seller_keys.clone()),
    )
    .await?;
    let IssueResponse {
        contract_id, iface, ..
    } = issuer_resp[0].clone();

    // 5. Create Seller Swap Side
    let contract_amount = 1;
    let seller_sk = seller_keys.private.nostr_prv.clone();
    let bitcoin_price: u64 = 100000;
    let seller_asset_desc = seller_keys.public.rgb_udas_descriptor_xpub.clone();
    let expire_at = (chrono::Local::now() + chrono::Duration::minutes(5))
        .naive_utc()
        .timestamp();
    let seller_swap_req = RgbOfferRequest {
        contract_id: contract_id.clone(),
        iface,
        contract_amount,
        bitcoin_price,
        descriptor: SecretString(seller_asset_desc),
        change_terminal: "/21/1".to_string(),
        bitcoin_changes: vec![],
        expire_at: Some(expire_at),
    };

    let seller_swap_resp = create_seller_offer(&seller_sk, seller_swap_req).await;
    assert!(seller_swap_resp.is_ok());

    // 7. Create Buyer Swap Side
    let RgbOfferResponse {
        offer_id,
        contract_amount,
        ..
    } = seller_swap_resp?;

    let buyer_sk = buyer_keys.private.nostr_prv.clone();
    let buyer_btc_desc = buyer_keys.public.btc_descriptor_xpub.clone();
    let buyer_swap_req = RgbBidRequest {
        offer_id: offer_id.clone(),
        asset_amount: contract_amount,
        descriptor: SecretString(buyer_btc_desc),
        change_terminal: "/1/0".to_string(),
        fee: PsbtFeeRequest::Value(1000),
    };

    let buyer_swap_resp = create_buyer_bid(&buyer_sk, buyer_swap_req).await;
    assert!(buyer_swap_resp.is_ok());

    // 8. Sign the Buyer Side
    let RgbBidResponse {
        bid_id, swap_psbt, ..
    } = buyer_swap_resp?;
    let request = SignPsbtRequest {
        psbt: swap_psbt,
        descriptors: vec![
            SecretString(buyer_keys.private.btc_descriptor_xprv.clone()),
            SecretString(buyer_keys.private.btc_change_descriptor_xprv.clone()),
        ],
    };
    let buyer_psbt_resp = sign_psbt_file(request).await;
    assert!(buyer_psbt_resp.is_ok());

    // 9. Create Swap PSBT
    let SignedPsbtResponse {
        psbt: swap_psbt, ..
    } = buyer_psbt_resp?;
    let final_swap_req = RgbSwapRequest {
        offer_id,
        bid_id,
        swap_psbt,
    };

    let final_swap_resp = create_swap_transfer(issuer_sk, final_swap_req).await;
    assert!(final_swap_resp.is_ok());

    // 8. Sign the Final PSBT
    let RgbSwapResponse {
        final_consig,
        final_psbt,
        ..
    } = final_swap_resp?;

    let request = SignPsbtRequest {
        psbt: final_psbt.clone(),
        descriptors: vec![
            SecretString(seller_keys.private.btc_descriptor_xprv.clone()),
            SecretString(seller_keys.private.btc_change_descriptor_xprv.clone()),
            SecretString(seller_keys.private.rgb_udas_descriptor_xprv.clone()),
        ],
    };
    let seller_psbt_resp = sign_and_publish_psbt_file(request).await;
    assert!(seller_psbt_resp.is_ok());

    // 9. Accept Consig (Buyer/Seller)
    let all_sks = [buyer_sk.clone(), seller_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: final_consig.clone(),
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 10 Mine Some Blocks
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    send_some_coins(whatever_address, "0.001").await;

    // 11. Retrieve Contract (Seller Side)
    let resp = get_contract(&seller_sk, &contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(0, resp?.balance);

    // 12. Retrieve Contract (Buyer Side)
    let resp = get_contract(&buyer_sk, &contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    // 13. Verify transfers (Seller Side)
    let resp = verify_transfers(&seller_sk).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.transfers.len());

    Ok(())
}

#[tokio::test]
async fn create_presig_scriptless_swap() -> anyhow::Result<()> {
    // 1. Initial Setup
    let seller_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let buyer_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let watcher_name = "default";
    let issuer_sk = &seller_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: seller_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = &buyer_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: buyer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(owner_sk, create_watch_req.clone()).await?;

    // 2. Setup Wallets (Seller)
    let btc_address_1 = get_new_address(
        &SecretString(seller_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let default_coins = "0.001";
    send_some_coins(&btc_address_1, default_coins).await;

    let btc_descriptor_xprv = SecretString(seller_keys.private.btc_descriptor_xprv.clone());
    let btc_change_descriptor_xprv =
        SecretString(seller_keys.private.btc_change_descriptor_xprv.clone());

    let assets_address_1 = get_new_address(
        &SecretString(seller_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let assets_address_2 = get_new_address(
        &SecretString(seller_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_1 = get_new_address(
        &SecretString(seller_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_2 = get_new_address(
        &SecretString(seller_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let btc_wallet = get_wallet(&btc_descriptor_xprv, Some(&btc_change_descriptor_xprv)).await?;
    sync_wallet(&btc_wallet).await?;

    let fund_vault = fund_vault(
        &btc_descriptor_xprv,
        &btc_change_descriptor_xprv,
        &assets_address_1,
        &assets_address_2,
        &uda_address_1,
        &uda_address_2,
        Some(1.1),
    )
    .await?;

    // 3. Send some coins (Buyer)
    let btc_address_1 = get_new_address(
        &SecretString(buyer_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;
    let asset_address_1 = get_new_address(
        &SecretString(buyer_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let default_coins = "0.1";
    send_some_coins(&btc_address_1, default_coins).await;
    send_some_coins(&asset_address_1, default_coins).await;

    // 4. Issue Contract (Seller)
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        5,
        false,
        false,
        None,
        None,
        Some(UtxoFilter::with_outpoint(
            fund_vault.assets_output.unwrap_or_default(),
        )),
        Some(seller_keys.clone()),
    )
    .await?;
    let IssueResponse {
        contract_id,
        iface,
        supply,
        ..
    } = issuer_resp[0].clone();

    // 5. Create Seller Swap Side
    let contract_amount = supply - 1;
    let seller_sk = seller_keys.private.nostr_prv.clone();
    let bitcoin_price: u64 = 100000;
    let seller_asset_desc = seller_keys.public.rgb_assets_descriptor_xpub.clone();
    let expire_at = (chrono::Local::now() + chrono::Duration::minutes(5))
        .naive_utc()
        .timestamp();
    let seller_swap_req = RgbOfferRequest {
        contract_id: contract_id.clone(),
        iface,
        contract_amount,
        bitcoin_price,
        descriptor: SecretString(seller_asset_desc),
        change_terminal: "/20/1".to_string(),
        bitcoin_changes: vec![],
        expire_at: Some(expire_at),
    };

    let seller_swap_resp = create_seller_offer(&seller_sk, seller_swap_req).await;
    assert!(seller_swap_resp.is_ok());

    // 6. Sign the Seller PSBT
    let RgbOfferResponse {
        offer_id,
        contract_amount,
        seller_psbt,
        ..
    } = seller_swap_resp?;

    let seller_psbt_req = SignPsbtRequest {
        psbt: seller_psbt.clone(),
        descriptors: vec![
            SecretString(seller_keys.private.btc_descriptor_xprv.clone()),
            SecretString(seller_keys.private.btc_change_descriptor_xprv.clone()),
            SecretString(seller_keys.private.rgb_assets_descriptor_xprv.clone()),
        ],
    };
    let seller_psbt_resp = sign_psbt_file(seller_psbt_req).await;
    assert!(seller_psbt_resp.is_ok());

    let SignedPsbtResponse { psbt, .. } = seller_psbt_resp?;
    let update_offer_req = RgbOfferUpdateRequest {
        contract_id: contract_id.clone(),
        offer_id: offer_id.clone(),
        offer_psbt: psbt,
    };
    let update_offer_resp = update_seller_offer(&seller_sk, update_offer_req).await;
    assert!(update_offer_resp.is_ok());

    // 7. Create Buyer Swap Side
    let buyer_sk = buyer_keys.private.nostr_prv.clone();
    let buyer_btc_desc = buyer_keys.public.btc_descriptor_xpub.clone();
    let buyer_swap_req = RgbBidRequest {
        offer_id: offer_id.clone(),
        asset_amount: contract_amount,
        descriptor: SecretString(buyer_btc_desc),
        change_terminal: "/1/0".to_string(),
        fee: PsbtFeeRequest::Value(1000),
    };

    let buyer_swap_resp = create_buyer_bid(&buyer_sk, buyer_swap_req).await;
    assert!(buyer_swap_resp.is_ok());

    // 8. Sign the Buyer Side
    let RgbBidResponse {
        bid_id, swap_psbt, ..
    } = buyer_swap_resp?;
    let request = SignPsbtRequest {
        psbt: swap_psbt,
        descriptors: vec![
            SecretString(buyer_keys.private.btc_descriptor_xprv.clone()),
            SecretString(buyer_keys.private.btc_change_descriptor_xprv.clone()),
        ],
    };
    let buyer_psbt_resp = sign_psbt_file(request).await;
    assert!(buyer_psbt_resp.is_ok());

    // 9. Create Swap PSBT
    let SignedPsbtResponse {
        psbt: swap_psbt, ..
    } = buyer_psbt_resp?;
    let final_swap_req = RgbSwapRequest {
        offer_id,
        bid_id,
        swap_psbt,
    };

    let final_swap_resp = create_swap_transfer(issuer_sk, final_swap_req).await;
    assert!(final_swap_resp.is_ok());

    // 10. Publish Swap PSBT
    let RgbSwapResponse {
        final_consig,
        final_psbt,
        ..
    } = final_swap_resp?;
    let final_swap_req = PublishPsbtRequest { psbt: final_psbt };
    let published_psbt_resp = publish_psbt_file(final_swap_req).await;
    assert!(published_psbt_resp.is_ok());

    // 11. Accept Consig (Buyer/Seller)
    let all_sks = [buyer_sk.clone(), seller_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: final_consig.clone(),
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 12 Mine Some Blocks
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    send_some_coins(whatever_address, "0.001").await;

    // 13. Retrieve Contract (Seller Side)
    let resp = get_contract(&seller_sk, &contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    // 14. Retrieve Contract (Buyer Side)
    let resp = get_contract(&buyer_sk, &contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(4, resp?.balance);

    // 15. Verify transfers (Seller Side)
    let resp = verify_transfers(&seller_sk).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.transfers.len());

    Ok(())
}
