#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::{
    bitcoin::{save_mnemonic, sign_psbt_file},
    rgb::{accept_transfer, get_contract},
    structs::{AcceptRequest, EncryptedWalletData, SignPsbtRequest},
};

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, import_new_contract,
    issuer_issue_contract, ISSUER_MNEMONIC, OWNER_MNEMONIC,
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
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, true, None).await;
    assert!(issuer_resp.is_ok());

    let import_resp = import_new_contract(issuer_resp?).await;
    assert!(import_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_get_fungible_contract_state_by_accept_cosign() -> anyhow::Result<()> {
    // 1. Issue and Generate Trasnfer (Issuer side)
    let issuer_keys: EncryptedWalletData = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let owner_keys = save_mnemonic(OWNER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract("RGB20", 5, false, true, None).await?;
    let owner_resp = create_new_invoice(issuer_resp.clone()).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp.clone()).await?;
    let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.clone().psbt,
        mnemonic: ISSUER_MNEMONIC.to_string(),
        seed_password: String::new(),
        iface: issuer_resp.iface,
    };
    let resp = sign_psbt_file(&issuer_sk, request).await;
    assert!(resp.is_ok());

    // 4. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: true,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 3. Accept Consig (Owner Side)
    let request = AcceptRequest {
        consignment: transfer_resp.consig,
        force: true,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Retrieve Contract (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(4, resp?.balance);

    // 5. Retrieve Contract (Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    Ok(())
}

#[tokio::test]
async fn allow_get_uda_contract_state_by_accept_cosign() -> anyhow::Result<()> {
    // 1. Issue and Generate Trasnfer (Issuer side)
    let issuer_keys: EncryptedWalletData = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let owner_keys = save_mnemonic(OWNER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, true, None).await?;
    let owner_resp = create_new_invoice(issuer_resp.clone()).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp.clone()).await?;
    let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

    // 2. Sign and Publish TX (Issuer side)
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.clone().psbt,
        mnemonic: ISSUER_MNEMONIC.to_string(),
        seed_password: String::new(),
        iface: issuer_resp.iface,
    };
    let resp = sign_psbt_file(&issuer_sk, request).await;
    assert!(resp.is_ok());

    // 4. Accept Consig (Issuer Side)
    let request = AcceptRequest {
        consignment: transfer_resp.clone().consig,
        force: true,
    };
    let resp = accept_transfer(&issuer_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 3. Accept Consig (Owner Side)
    let request = AcceptRequest {
        consignment: transfer_resp.consig,
        force: true,
    };
    let resp = accept_transfer(&owner_sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    // 4. Retrieve Contract (Issuer Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&issuer_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(0, resp?.balance);

    // 5. Retrieve Contract (Owner Side)
    let contract_id = &issuer_resp.contract_id;
    let resp = get_contract(&owner_sk, contract_id).await;
    assert!(resp.is_ok());
    assert_eq!(1, resp?.balance);

    Ok(())
}
