#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::{
    bitcoin::{save_mnemonic, sign_and_publish_psbt_file},
    rgb::{accept_transfer, create_watcher, get_contract},
    structs::{AcceptRequest, DecryptedWalletData, SecretString, SignPsbtRequest, WatcherRequest},
};

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, get_uda_data, import_new_contract,
    issuer_issue_contract, issuer_issue_contract_v2, send_some_coins, UtxoFilter, ISSUER_MNEMONIC,
    OWNER_MNEMONIC,
};

#[tokio::test]
async fn allow_import_fungible_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract("RGB20", 5, false, true, None).await;
    assert!(issuer_resp.is_ok());

    let import_resp = import_new_contract(issuer_resp?).await;
    assert!(import_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_import_uda_contract() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, true, single).await;
    assert!(issuer_resp.is_ok());

    let import_resp = import_new_contract(issuer_resp?).await;
    assert!(import_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn check_fungible_state_after_accept_consig() -> anyhow::Result<()> {
    // 1. Issue and Generate Trasnfer (Issuer side)
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys: DecryptedWalletData = save_mnemonic(
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
        5,
        false,
        true,
        None,
        Some("0.1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(10000000)),
        None,
    )
    .await?;
    let issuer_resp = &issuer_resp[0];

    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.legacy),
    )
    .await?;
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let owner_sk = owner_keys.clone().private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_and_publish_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 3. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: true,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Accept Consig (Owner Side)
    let request = AcceptRequest {
        consignment: transfer_resp.consig.clone(),
        force: false,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 5. Retrieve Contract (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(4, resp?.balance);

    // 6. Create Watcher (Owner Side)
    let watcher_name = "default";
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.clone().public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req).await?;

    // 7. Retrieve Contract (Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    Ok(())
}

#[tokio::test]
async fn check_uda_state_after_accept_consig() -> anyhow::Result<()> {
    // 1. Issue and Generate Trasnfer (Issuer side)
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys: DecryptedWalletData = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = &save_mnemonic(
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
        Some("0.1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(10000000)),
        None,
    )
    .await?;
    let issuer_resp = issuer_resp[0].clone();
    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.legacy),
    )
    .await?;
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let owner_sk = owner_keys.clone().private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            issuer_keys.private.rgb_udas_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_and_publish_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 3. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.consig.clone(),
        force: false,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Accept Consig (Owner Side)
    let request = AcceptRequest {
        consignment: transfer_resp.consig.clone(),
        force: false,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 5. Retrieve Contract (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(0, resp?.balance);

    // 6. Create Watcher (Owner Side)
    let watcher_name = "default";
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.clone().public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req).await?;

    // 7. Retrieve Contract (Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    Ok(())
}
