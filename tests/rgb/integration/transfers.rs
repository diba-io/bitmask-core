#![cfg(not(target_arch = "wasm32"))]
use std::collections::{BTreeSet, HashMap};

use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, new_mnemonic, save_mnemonic, sign_psbt_file, sync_wallet},
    rgb::{
        accept_transfer, create_invoice, create_watcher, full_transfer_asset, get_contract, import,
        save_transfer, verify_transfers, watcher_next_address, watcher_next_utxo,
        watcher_unspent_utxos,
    },
    structs::{
        AcceptRequest, AllocationDetail, AssetType, DecryptedWalletData, FullRgbTransferRequest,
        ImportRequest, InvoiceRequest, IssueResponse, PsbtFeeRequest, RgbSaveTransferRequest,
        RgbTransferResponse, SecretString, SignPsbtRequest, WatcherRequest,
    },
};

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_invoice_v2, create_new_psbt, create_new_psbt_v2,
    create_new_transfer, get_uda_data, issuer_issue_contract_v2, send_some_coins, UtxoFilter,
    ANOTHER_OWNER_MNEMONIC, ISSUER_MNEMONIC, OWNER_MNEMONIC,
};

#[tokio::test]
async fn allow_issuer_make_conseq_transfers() -> anyhow::Result<()> {
    // 0. Retrieve all keys
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

    // 1. Create All Watchers
    let watcher_name = "default";
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req.clone()).await?;

    // 2. Issuer Contract
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

    // 3. Owner Create Invoice
    let owner_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 4. Create First Transfer
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        issuer_keys.clone(),
        owner_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;

    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 5. Accept Consig (Issuer and Owner Side)
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: transfer_resp.clone().consig,
            force: false,
        };
        let accept_resp = accept_transfer(&sk, request).await;
        assert!(accept_resp.is_ok());
        assert!(accept_resp?.valid);
    }

    // 6. Check Contract Balances (Issuer Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(3, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(2, owner_contract.balance);

    // 7. reate new Invoices (Issuer Side)
    let address = watcher_next_address(&owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&address.address, "0.1").await;
    send_some_coins(&address.address, "0.1").await;

    let utxos = watcher_unspent_utxos(&owner_sk, watcher_name, "RGB20").await?;
    let invoice_2 = &create_new_invoice_v2(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        &utxos.utxos[0].outpoint,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 8. Create Transfer and Accept (Issuer Side)
    let issuer_utxo = issuer_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &issuer_contract.contract_id,
        &issuer_contract.iface,
        vec![issuer_utxo.utxo],
        issuer_keys.clone(),
    )
    .await?;
    let issuer_transfer_to_another_resp =
        &create_new_transfer(issuer_keys.clone(), invoice_2.clone(), psbt_resp.clone()).await?;
    let request: SignPsbtRequest = SignPsbtRequest {
        psbt: issuer_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 9. Accept Consig (Issuer and Another Owner Sides)
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: issuer_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 10. Accept Consig (Owner and Another Owner Sides)
    let all_sks = [owner_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: issuer_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }
    Ok(())
}

#[tokio::test]
async fn allow_owner_make_conseq_transfers() -> anyhow::Result<()> {
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

    // 1. Issue and Create First Transfer (Issuer side)
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
        2,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
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
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    // 2. Sign and Publish TX (Issuer side)
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 3. Accept Consig (Issuer Side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Create Watcher (Owner Side)
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: "default".to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req.clone()).await?;

    // 5. Accept Consig (Owner Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: true,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 6. Get Contract (Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(2, owner_contract.balance);

    // 7. Create Invoice (Issuer Side)
    let issuer_invoice_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        issuer_keys.clone(),
        None,
        None,
    )
    .await?;

    // 8. Create Transfer and Accept (Issuer Side)
    let contract_utxo = owner_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &owner_contract.contract_id,
        &owner_contract.iface,
        vec![contract_utxo.utxo],
        owner_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        owner_keys.clone(),
        issuer_invoice_resp.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            owner_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 9. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 10. Check Contract Balance
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    // 11. Create Invoice (Issuer Side)
    let issuer_invoice_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        issuer_keys.clone(),
        None,
        None,
    )
    .await?;

    // 12. Create Transfer and Accept (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    let contract_utxo = owner_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &owner_contract.contract_id,
        &owner_contract.iface,
        vec![contract_utxo.utxo],
        owner_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        owner_keys.clone(),
        issuer_invoice_resp.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            owner_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 13. Accept Consig (Owner Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 14. Check Contract Balance (Owner Side)
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(0, owner_contract.balance);
    Ok(())
}

#[tokio::test]
async fn allow_conseq_transfers_between_tree_owners() -> anyhow::Result<()> {
    // 0. Retrieve all keys
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
    let another_owner_keys = &save_mnemonic(
        &SecretString(ANOTHER_OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // 1. Create All Watchers
    let watcher_name = "default";
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req.clone()).await?;

    let another_owner_sk = another_owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: another_owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&another_owner_sk, create_watch_req.clone()).await?;

    // 2. Issuer Contract
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

    // 3. Owner Create Invoice
    let owner_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 4. Create First Transfer
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        issuer_keys.clone(),
        owner_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;

    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 5. Accept Consig (Issuer and Owner Side)
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: transfer_resp.clone().consig,
            force: false,
        };
        let accept_resp = accept_transfer(&sk, request).await;
        assert!(accept_resp.is_ok());
        assert!(accept_resp?.valid);
    }

    // 6. Check Contract Balances (Issuer Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(3, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(2, owner_contract.balance);

    // 7. Create 2 Invoices (Another Owner Side)
    let another_invoice_1 = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        another_owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    let another_invoice_2 = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        another_owner_keys.clone(),
        None,
        None,
    )
    .await?;

    // 8. Create Transfer and Accept (Issuer Side)
    let issuer_xpriv = issuer_keys.private.rgb_assets_descriptor_xprv.clone();
    let issuer_utxo = issuer_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &issuer_contract.contract_id,
        &issuer_contract.iface,
        vec![issuer_utxo.utxo],
        issuer_keys.clone(),
    )
    .await?;
    let issuer_transfer_to_another_resp = &create_new_transfer(
        issuer_keys.clone(),
        another_invoice_1.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: issuer_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(issuer_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.001").await;

    // 9. Create Transfer and Accept (Owner Side)
    let owner_xpriv = owner_keys.private.rgb_assets_descriptor_xprv.clone();
    let owner_utxo = owner_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &owner_contract.contract_id,
        &owner_contract.iface,
        vec![owner_utxo.utxo],
        owner_keys.clone(),
    )
    .await?;
    let owner_transfer_to_another_resp = &create_new_transfer(
        owner_keys.clone(),
        another_invoice_2.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: owner_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(owner_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 9. Accept Consig (Issuer and Another Owner Sides)
    let all_sks = [issuer_sk.clone(), another_owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: issuer_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 10. Accept Consig (Owner and Another Owner Sides)
    let all_sks = [owner_sk.clone(), another_owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: owner_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 11. Verify Balances (All Sides)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(2, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    let resp = get_contract(&another_owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let another_owner_contract = resp?;
    assert_eq!(2, another_owner_contract.balance);

    Ok(())
}

#[tokio::test]
async fn allows_spend_amount_from_two_different_owners() -> anyhow::Result<()> {
    // 0. Retrieve all keys
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
    let another_owner_keys = &save_mnemonic(
        &SecretString(ANOTHER_OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // 1. Create All Watchers
    let watcher_name = "default";
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&owner_sk, create_watch_req.clone()).await?;

    let another_owner_sk = another_owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: another_owner_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&another_owner_sk, create_watch_req.clone()).await?;

    // 2. Issuer Contract
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

    // 3. Owner Create Invoice
    let owner_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 4. Create First Transfer
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        issuer_keys.clone(),
        owner_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;

    let issuer_xprv = issuer_keys.private.rgb_assets_descriptor_xprv.clone();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(issuer_xprv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 5. Accept Consig (Issuer and Owner Side)
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: transfer_resp.clone().consig,
            force: false,
        };
        let accept_resp = accept_transfer(&sk, request).await;
        assert!(accept_resp.is_ok());
        assert!(accept_resp?.valid);
    }

    // 6. Check Contract Balances (Issuer Owner Side)
    let another_owner_address =
        watcher_next_address(&another_owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&another_owner_address.address, "0.1").await;

    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(3, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    // 7. Create 2 Invoices (Another Owner Side)
    let another_owner_address =
        watcher_next_address(&another_owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&another_owner_address.address, "0.1").await;

    let another_owner_utxo = watcher_next_utxo(&another_owner_sk, watcher_name, "RGB20").await?;
    let another_owner_utxo = another_owner_utxo.utxo.unwrap().outpoint;
    let another_owner_seal = format!("tapret1st:{another_owner_utxo}");
    let invoice_req = InvoiceRequest {
        contract_id: contract_id.to_owned(),
        iface: issuer_resp.iface.to_owned(),
        amount: 1,
        seal: another_owner_seal,
        params: HashMap::default(),
    };
    let import_req = ImportRequest {
        import: AssetType::RGB20,
        data: issuer_resp.contract.strict.clone(),
    };

    let resp = import(&another_owner_sk, import_req).await;
    assert!(resp.is_ok());

    let another_invoice_1 = &create_invoice(&another_owner_sk, invoice_req.clone()).await?;
    let another_invoice_2 = &create_invoice(&another_owner_sk, invoice_req.clone()).await?;

    // 8. Create Transfer and Accept (Issuer Side)
    let issuer_xpriv = issuer_keys.private.rgb_assets_descriptor_xprv.clone();
    let issuer_utxo = issuer_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &issuer_contract.contract_id,
        &issuer_contract.iface,
        vec![issuer_utxo.utxo],
        issuer_keys.clone(),
    )
    .await?;
    let issuer_transfer_to_another_resp = &create_new_transfer(
        issuer_keys.clone(),
        another_invoice_1.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: issuer_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(issuer_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 9. Create Transfer and Accept (Owner Side)
    let owner_xpriv = owner_keys.private.rgb_assets_descriptor_xprv.clone();
    let owner_utxo = owner_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &owner_contract.contract_id,
        &owner_contract.iface,
        vec![owner_utxo.utxo],
        owner_keys.clone(),
    )
    .await?;
    let owner_transfer_to_another_resp = &create_new_transfer(
        owner_keys.clone(),
        another_invoice_2.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: owner_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(owner_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 9. Accept Consig (Issuer and Another Owner Sides)
    let all_sks = [issuer_sk.clone(), another_owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: issuer_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 10. Accept Consig (Owner and Another Owner Sides)
    let all_sks = [owner_sk.clone(), another_owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: owner_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 11. Verify Balances (All Sides)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(2, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    let resp = get_contract(&another_owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let another_owner_contract = resp?;
    assert_eq!(2, another_owner_contract.balance);

    // 12. Generate Invoice (Issuer Side)
    let issuer_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        issuer_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 13. Spend All Funds (Another Owner Side)
    let another_owner_xpriv = another_owner_keys
        .private
        .rgb_assets_descriptor_xprv
        .clone();
    let psbt_resp = create_new_psbt(
        &owner_contract.contract_id,
        &owner_contract.iface,
        vec![another_owner_utxo],
        another_owner_keys.clone(),
    )
    .await?;
    let another_transfer_to_issuer = &create_new_transfer(
        another_owner_keys.clone(),
        issuer_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: another_transfer_to_issuer.psbt.clone(),
        descriptors: [SecretString(another_owner_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 14. Accept Consig (Another Owner Side)
    let request = AcceptRequest {
        consignment: another_transfer_to_issuer.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&another_owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 15. Accept Consig (Another Owner Side)
    let resp = get_contract(&another_owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let another_owner_contract = resp?;
    assert_eq!(0, another_owner_contract.balance);

    Ok(())
}

#[tokio::test]
async fn allows_spend_amount_from_two_different_transitions() -> anyhow::Result<()> {
    // 0. Retrieve all keys
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
    let another_owner_keys = &save_mnemonic(
        &SecretString(ANOTHER_OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // 1. Create All Watchers
    let watcher_name = "default";
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req.clone()).await?;

    let another_owner_sk = another_owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: another_owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&another_owner_sk, create_watch_req.clone()).await?;

    // 2. Issuer Contract
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

    // 3. Owner Create Invoice
    let owner_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 4. Create First Transfer
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        issuer_keys.clone(),
        owner_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;

    let issuer_xprv = issuer_keys.private.rgb_assets_descriptor_xprv.clone();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: [SecretString(issuer_xprv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 5. Accept Consig (Issuer and Owner Side)
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: transfer_resp.clone().consig,
            force: false,
        };
        let accept_resp = accept_transfer(&sk, request).await;
        assert!(accept_resp.is_ok());
        assert!(accept_resp?.valid);
    }

    // 6. Check Contract Balances (Issuer Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(3, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(2, owner_contract.balance);

    // 7. reate new Invoices (Issuer Side)
    let address = watcher_next_address(&another_owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&address.address, "0.1").await;
    send_some_coins(&address.address, "0.1").await;

    let utxos = watcher_unspent_utxos(&another_owner_sk, watcher_name, "RGB20").await?;
    let another_invoice_1 = &create_new_invoice_v2(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        &utxos.utxos[0].outpoint,
        another_owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    let another_invoice_2 = &create_new_invoice_v2(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        &utxos.utxos[1].outpoint,
        another_owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 8. Create Transfer and Accept (Issuer Side)
    let issuer_xpriv = issuer_keys.private.rgb_assets_descriptor_xprv.clone();
    let issuer_utxo = issuer_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &issuer_contract.contract_id,
        &issuer_contract.iface,
        vec![issuer_utxo.utxo],
        issuer_keys.clone(),
    )
    .await?;
    let issuer_transfer_to_another_resp = &create_new_transfer(
        issuer_keys.clone(),
        another_invoice_1.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: issuer_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(issuer_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 9. Create Transfer and Accept (Owner Side)
    let owner_xpriv = owner_keys.private.rgb_assets_descriptor_xprv.clone();
    let owner_utxo = owner_contract
        .allocations
        .into_iter()
        .find(|alloc| alloc.is_mine && !alloc.is_spent)
        .unwrap();

    let psbt_resp = create_new_psbt(
        &owner_contract.contract_id,
        &owner_contract.iface,
        vec![owner_utxo.utxo],
        owner_keys.clone(),
    )
    .await?;
    let owner_transfer_to_another_resp = &create_new_transfer(
        owner_keys.clone(),
        another_invoice_2.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: owner_transfer_to_another_resp.psbt.clone(),
        descriptors: [SecretString(owner_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 9. Accept Consig (Issuer and Another Owner Sides)
    let all_sks = [issuer_sk.clone(), another_owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: issuer_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 10. Accept Consig (Owner and Another Owner Sides)
    let all_sks = [owner_sk.clone(), another_owner_sk.clone()];
    for sk in all_sks {
        let request = AcceptRequest {
            consignment: owner_transfer_to_another_resp.clone().consig,
            force: false,
        };
        let resp = accept_transfer(&sk, request).await;
        assert!(resp.is_ok());
        assert!(resp?.valid);
    }

    // 11. Verify Balances (All Sides)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(2, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    let resp = get_contract(&another_owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let another_owner_contract = resp?;
    assert_eq!(2, another_owner_contract.balance);

    // 12. Generate Invoice (Issuer Side)
    let issuer_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        issuer_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    let allocs: BTreeSet<AllocationDetail> = another_owner_contract
        .allocations
        .into_iter()
        .filter(|alloc| alloc.is_mine && !alloc.is_spent)
        .collect();
    assert_eq!(2, allocs.len());

    // 13. Spend All Funds (Another Owner Side)
    let another_owner_xpriv = another_owner_keys
        .private
        .rgb_assets_descriptor_xprv
        .clone();

    let psbt_resp = create_new_psbt_v2(
        &owner_contract.iface,
        allocs.into_iter().collect(),
        another_owner_keys.clone(),
        vec![],
        vec![],
        None,
    )
    .await?;
    let another_transfer_to_issuer = &create_new_transfer(
        another_owner_keys.clone(),
        issuer_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;
    let request = SignPsbtRequest {
        psbt: another_transfer_to_issuer.psbt.clone(),
        descriptors: [SecretString(another_owner_xpriv)].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 14. Accept Consig (Another Owner Side)
    let request = AcceptRequest {
        consignment: another_transfer_to_issuer.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&another_owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 15. Get Contract (Another Owner Side)
    let resp = get_contract(&another_owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let another_owner_contract = resp?;
    assert_eq!(0, another_owner_contract.balance);

    Ok(())
}

#[tokio::test]
async fn allow_issuer_make_transfer_of_two_contracts_in_same_utxo() -> anyhow::Result<()> {
    // 1. Issue and First Transfer (Issuer side)
    let whatever_address = "bcrt1p76gtucrxhmn8s5622r859dpnmkj0kgfcel9xy0sz6yj84x6ppz2qk5hpsw";
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = &save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issue_contracts_resp = &issuer_issue_contract_v2(
        2,
        "RGB20",
        5,
        false,
        true,
        None,
        Some("1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(100000000)),
        None,
    )
    .await?;
    let issue_contract_a_resp = issue_contracts_resp[0].clone();
    let issue_contract_b_resp = issue_contracts_resp[1].clone();

    // 2. Create First Invoice (Owner Side)
    let watcher_name = "default";
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: false,
    };
    create_watcher(&owner_sk, create_watch_req).await?;
    let owner_address = watcher_next_address(&owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&owner_address.address, "1").await;

    let owner_utxos = watcher_unspent_utxos(&owner_sk, watcher_name, "RGB20").await?;
    let owner_resp = &create_new_invoice_v2(
        &issue_contract_a_resp.contract_id,
        &issue_contract_a_resp.iface,
        1,
        &owner_utxos.utxos[0].outpoint,
        owner_keys.clone(),
        None,
        Some(issue_contract_a_resp.clone().contract.strict),
    )
    .await?;

    let psbt_resp = create_new_psbt(
        &issue_contract_a_resp.contract_id,
        &issue_contract_a_resp.iface,
        vec![issue_contract_a_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 3. Accept Consig (Both Sides)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&issuer_sk, request.clone()).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Check Contract Balance (Both Sides)
    send_some_coins(&owner_address.address, "1").await;
    let contract_id = &issue_contract_a_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(4, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    // 5. Create Second Invoice (Owner Side)
    let owner_resp = &create_new_invoice_v2(
        &issue_contract_b_resp.contract_id,
        &issue_contract_b_resp.iface,
        2,
        &owner_utxos.utxos[0].outpoint,
        owner_keys.clone(),
        None,
        Some(issue_contract_b_resp.clone().contract.strict),
    )
    .await?;

    // 6. Create Second Transfer (Issuer Side)
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine && !x.is_spent)
        .unwrap();
    let psbt_resp = create_new_psbt_v2(
        &issue_contract_b_resp.iface,
        vec![new_alloc],
        issuer_keys.clone(),
        vec![],
        vec![],
        Some(bitmask_core::structs::PsbtFeeRequest::Value(10000)),
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp).await?;
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());
    send_some_coins(whatever_address, "0.1").await;

    // 7. Accept Consig (Both Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: true,
    };
    let resp = accept_transfer(&issuer_sk, request.clone()).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 8. Check Contract Balance (Both Sides)
    send_some_coins(&owner_address.address, "0.1").await;
    let contract_id = &issue_contract_b_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(3, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(2, owner_contract.balance);

    Ok(())
}

#[tokio::test]
async fn allow_issuer_make_transfer_of_two_contract_types_in_same_utxo() -> anyhow::Result<()> {
    // 1. Issue and First Transfer (Issuer side)
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = &save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issue_contracts_resp = &issuer_issue_contract_v2(
        1,
        "RGB20",
        5,
        false,
        true,
        None,
        Some("1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(100000000)),
        None,
    )
    .await?;
    let issue_contract_a_resp = issue_contracts_resp[0].clone();

    let meta = Some(get_uda_data());
    let issue_contracts_resp = &issuer_issue_contract_v2(
        1,
        "RGB21",
        1,
        false,
        true,
        meta,
        Some("1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(100000000)),
        None,
    )
    .await?;
    let issue_contract_b_resp = issue_contracts_resp[0].clone();

    // 2. Create First Invoice (Owner Side)
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

    let owner_uda_address = watcher_next_address(&owner_sk, watcher_name, "RGB21").await?;
    send_some_coins(&owner_uda_address.address, "1").await;

    let owner_utxos = watcher_unspent_utxos(&owner_sk, watcher_name, "RGB20").await?;
    let owner_resp = &create_new_invoice_v2(
        &issue_contract_a_resp.contract_id,
        &issue_contract_a_resp.iface,
        1,
        &owner_utxos.utxos[0].outpoint,
        owner_keys.clone(),
        None,
        Some(issue_contract_a_resp.clone().contract.strict),
    )
    .await?;

    let psbt_resp = create_new_psbt(
        &issue_contract_a_resp.contract_id,
        &issue_contract_a_resp.iface,
        vec![issue_contract_a_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )],
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 3. Accept Consig (Both Sides)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&issuer_sk, request.clone()).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Check Contract Balance (Both Sides)
    send_some_coins(&owner_fungible_address.address, "1").await;
    send_some_coins(&owner_uda_address.address, "1").await;
    let contract_id = &issue_contract_a_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(4, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    // 5. Create Second Invoice (Owner Side)
    let owner_utxos = watcher_unspent_utxos(&owner_sk, watcher_name, "RGB21").await?;
    let owner_resp = &create_new_invoice_v2(
        &issue_contract_b_resp.contract_id,
        &issue_contract_b_resp.iface,
        1,
        &owner_utxos.utxos[0].outpoint,
        owner_keys.clone(),
        None,
        Some(issue_contract_b_resp.clone().contract.strict),
    )
    .await?;

    // 6. Create Second Transfer (Issuer Side)
    let resp = get_contract(&issuer_sk, &issue_contract_b_resp.contract_id).await;
    let issuer_contract = resp?;
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine && !x.is_spent)
        .unwrap();
    let psbt_resp = create_new_psbt_v2(
        &issue_contract_b_resp.iface,
        vec![new_alloc],
        issuer_keys.clone(),
        vec![],
        vec![],
        Some(PsbtFeeRequest::Value(10000)),
    )
    .await?;
    let transfer_resp =
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp).await?;
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_udas_descriptor_xprv.clone(),
        )],
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 7. Accept Consig (Both Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: true,
    };
    let resp = accept_transfer(&issuer_sk, request.clone()).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 8. Check Contract Balance (Both Sides)
    send_some_coins(&owner_uda_address.address, "0.1").await;
    let contract_id = &issue_contract_b_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(0, issuer_contract.balance);

    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());

    let owner_contract = resp?;
    assert_eq!(1, owner_contract.balance);

    Ok(())
}

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
    for _ in 1..10 {
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
        // println!("Payment A #{i}:1 ({})", full_transfer_resp.is_ok());

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

        for _ in 1..2 {
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
            // println!("Payment B #{i}:{j} ({})", full_transfer_resp.is_ok());

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
