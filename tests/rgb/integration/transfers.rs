#![cfg(not(target_arch = "wasm32"))]
use std::collections::{BTreeSet, HashMap};

use bitmask_core::{
    bitcoin::{save_mnemonic, sign_psbt_file},
    rgb::{
        accept_transfer, create_invoice, create_watcher, get_contract, import,
        watcher_next_address, watcher_next_utxo, watcher_unspent_utxo,
    },
    structs::{
        AcceptRequest, AllocationDetail, AssetType, DecryptedWalletData, ImportRequest,
        InvoiceRequest, SecretString, SignPsbtRequest, WatcherRequest,
    },
};

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_invoice_v2, create_new_psbt, create_new_psbt_v2,
    create_new_transfer, issuer_issue_contract, send_some_coins, ANOTHER_OWNER_MNEMONIC,
    ISSUER_MNEMONIC, OWNER_MNEMONIC,
};

#[tokio::test]
async fn allow_issuer_make_conseq_transfers() -> anyhow::Result<()> {
    // 1. Issue and First Transfer (Issuer side)
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
    let issuer_resp = &issuer_issue_contract("RGB20", 5, false, true, None).await?;
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
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptor: SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 3. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Check Contract Balance (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());

    let issuer_contract = resp?;
    assert_eq!(4, issuer_contract.balance);

    // 5. Create Second Invoice (Owner Side)
    let owner_resp = create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys.clone(),
        None,
        None,
    )
    .await?;

    // 6. Create Second Transfer (Issuer Side)
    let new_alloc = issuer_contract
        .allocations
        .into_iter()
        .find(|x| x.is_mine == true)
        .unwrap();
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![new_alloc.utxo],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptor: SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 7. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: false,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 8. Check Contract Balance (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(3, resp?.balance);

    Ok(())
}

#[tokio::test]
async fn allow_owner_make_conseq_transfers() -> anyhow::Result<()> {
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
    let issuer_resp = &issuer_issue_contract("RGB20", 5, false, true, None).await?;
    let owner_resp = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
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
        &create_new_transfer(issuer_keys.clone(), owner_resp.clone(), psbt_resp.clone()).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_xprv = issuer_keys.private.rgb_assets_descriptor_xprv.clone();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptor: SecretString(issuer_xprv),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
        force: false,
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
        descriptor: SecretString(owner_keys.private.rgb_assets_descriptor_xprv.clone()),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
        descriptor: SecretString(owner_keys.private.rgb_assets_descriptor_xprv.clone()),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
    let issuer_resp = &issuer_issue_contract("RGB20", 5, false, true, None).await?;

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
        descriptor: SecretString(issuer_xprv),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
        descriptor: SecretString(issuer_xpriv),
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
        descriptor: SecretString(owner_xpriv),
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
    let issuer_resp = &issuer_issue_contract("RGB20", 5, false, true, None).await?;

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
        descriptor: SecretString(issuer_xprv),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
    let another_owner_address =
        watcher_next_address(&another_owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&another_owner_address.address, "0.1").await;

    let another_owner_utxo = watcher_next_utxo(&another_owner_sk, watcher_name, "RGB20").await?;
    let another_owner_seal = format!("tapret1st:{}", another_owner_utxo.utxo);
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
        descriptor: SecretString(issuer_xpriv),
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
        descriptor: SecretString(owner_xpriv),
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
    let another_owner_utxo = another_owner_utxo.utxo;

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
        descriptor: SecretString(another_owner_xpriv),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
    let issuer_resp = &issuer_issue_contract("RGB20", 5, false, true, None).await?;

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
        descriptor: SecretString(issuer_xprv),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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

    let utxos = watcher_unspent_utxo(&another_owner_sk, watcher_name, "RGB20").await?;
    let another_invoice_1 = &create_new_invoice_v2(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        &utxos.utxos[0],
        another_owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    let another_invoice_2 = &create_new_invoice_v2(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        &utxos.utxos[1],
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
        descriptor: SecretString(issuer_xpriv),
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
        descriptor: SecretString(owner_xpriv),
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

    let allocs: BTreeSet<AllocationDetail> = another_owner_contract
        .allocations
        .into_iter()
        .filter(|alloc| alloc.is_mine && !alloc.is_spent)
        .map(|f| f)
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
        descriptor: SecretString(another_owner_xpriv),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

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
