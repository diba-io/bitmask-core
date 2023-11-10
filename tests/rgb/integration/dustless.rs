#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt_v2, create_new_transfer, issuer_issue_contract_v2,
    send_some_coins, UtxoFilter,
};
use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{
        fund_vault, get_new_address, get_wallet, new_mnemonic, sign_and_publish_psbt_file,
        sync_wallet,
    },
    rgb::{
        accept_transfer, create_watcher, full_transfer_asset, get_contract, structs::ContractAmount,
    },
    structs::{
        AcceptRequest, FullRgbTransferRequest, PsbtFeeRequest, PsbtInputRequest, SecretString,
        SignPsbtRequest, WatcherRequest,
    },
};

#[tokio::test]
async fn create_dustless_transfer_with_fee_value() -> anyhow::Result<()> {
    // 1. Initial Setup
    let issuer_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let owner_keys = new_mnemonic(&SecretString("".to_string())).await?;

    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        ContractAmount::new(5, 2).to_value(),
        false,
        true,
        None,
        Some("0.00001".to_string()),
        Some(UtxoFilter::with_amount_equal_than(1000)),
        Some(issuer_keys.clone()),
    )
    .await?;
    let issuer_resp = issuer_resp[0].clone();
    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1.0,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 2. Get UTXO RGB with insufficient stats
    let issuer_sk = &issuer_keys.private.nostr_prv;
    let contract_id = &issuer_resp.contract_id;
    let issuer_contract = get_contract(issuer_sk, contract_id).await?;
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine)
        .unwrap();
    let allocs = [new_alloc];

    // 3. Get Bitcoin UTXO
    let issuer_btc_desc = &issuer_keys.public.btc_descriptor_xpub;
    let issuer_vault = get_wallet(&SecretString(issuer_btc_desc.to_string()), None).await?;
    let issuer_address = &issuer_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(issuer_address, "0.1").await;
    sync_wallet(&issuer_vault).await?;

    let btc_utxo = issuer_vault.lock().await.list_unspent()?;
    let btc_utxo = btc_utxo.first().unwrap();
    let bitcoin_inputs = [PsbtInputRequest {
        descriptor: SecretString(issuer_btc_desc.to_owned()),
        utxo: btc_utxo.outpoint.to_string(),
        utxo_terminal: "/0/0".to_string(),
        ..Default::default()
    }];

    // 2. Create PSBT
    let psbt_resp = create_new_psbt_v2(
        &issuer_resp.iface,
        allocs.to_vec(),
        issuer_keys.clone(),
        bitcoin_inputs.to_vec(),
        vec![],
        None,
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp).await?;

    let sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [
            SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
            SecretString(issuer_keys.private.btc_descriptor_xprv.clone()),
        ]
        .to_vec(),
    };
    let resp = sign_and_publish_psbt_file(request).await;
    assert!(resp.is_ok());

    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    send_some_coins(whatever_address, "0.1").await;

    let request = AcceptRequest {
        consignment: transfer_resp.consig.clone(),
        force: false,
    };

    let resp = accept_transfer(&sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);
    Ok(())
}

#[tokio::test]
async fn create_dustless_transfer_with_fee_rate() -> anyhow::Result<()> {
    // 1. Initial Setup
    let issuer_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let owner_keys = new_mnemonic(&SecretString("".to_string())).await?;

    // Create Watcher
    let watcher_name = "default";
    let issuer_sk = &issuer_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };

    create_watcher(issuer_sk, create_watch_req.clone()).await?;

    let btc_address_1 = get_new_address(
        &SecretString(issuer_keys.public.btc_descriptor_xpub.clone()),
        None,
    )
    .await?;

    // Min amount of satoshis
    let default_coins = "0.00010000";
    send_some_coins(&btc_address_1, default_coins).await;

    let btc_descriptor_xprv = SecretString(issuer_keys.private.btc_descriptor_xprv.clone());
    let btc_change_descriptor_xprv =
        SecretString(issuer_keys.private.btc_change_descriptor_xprv.clone());

    let assets_address_1 = get_new_address(
        &SecretString(issuer_keys.public.rgb_assets_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let uda_address_1 = get_new_address(
        &SecretString(issuer_keys.public.rgb_udas_descriptor_xpub.clone()),
        None,
    )
    .await?;

    let btc_wallet = get_wallet(&btc_descriptor_xprv, Some(&btc_change_descriptor_xprv)).await?;
    sync_wallet(&btc_wallet).await?;

    let fund_vault = fund_vault(
        &btc_descriptor_xprv,
        &btc_change_descriptor_xprv,
        &assets_address_1,
        &uda_address_1,
        Some(1.1),
    )
    .await?;

    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    send_some_coins(whatever_address, default_coins).await;

    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        ContractAmount::new(5, 2).to_value(),
        false,
        false,
        None,
        None,
        Some(UtxoFilter::with_outpoint(
            fund_vault.assets_output.unwrap_or_default(),
        )),
        Some(issuer_keys.clone()),
    )
    .await?;
    let issuer_resp = issuer_resp[0].clone();
    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1.0,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    let sk = issuer_keys.private.nostr_prv.to_string();
    let request = FullRgbTransferRequest {
        contract_id: issuer_resp.contract_id,
        iface: issuer_resp.iface,
        rgb_invoice: owner_resp.invoice.to_string(),
        descriptor: SecretString(issuer_keys.public.rgb_assets_descriptor_xpub.to_string()),
        change_terminal: "/20/1".to_string(),
        fee: PsbtFeeRequest::FeeRate(1.1),
        bitcoin_changes: vec![],
    };

    let transfer_resp = full_transfer_asset(&sk, request).await?;
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [
            SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
            SecretString(issuer_keys.private.btc_descriptor_xprv.clone()),
            SecretString(issuer_keys.private.btc_change_descriptor_xprv.clone()),
        ]
        .to_vec(),
    };
    let resp = sign_and_publish_psbt_file(request).await;
    assert!(resp.is_ok());

    let request = AcceptRequest {
        consignment: transfer_resp.consig.clone(),
        force: false,
    };

    let resp = accept_transfer(&sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);
    Ok(())
}
