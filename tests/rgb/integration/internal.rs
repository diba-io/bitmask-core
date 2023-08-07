#![cfg(not(target_arch = "wasm32"))]

use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, save_mnemonic, sync_wallet},
    rgb::full_transfer_asset,
    structs::{FullRgbTransferRequest, PsbtFeeRequest, SecretString},
};

use crate::rgb::integration::utils::{
    create_new_invoice, get_uda_data, issuer_issue_contract_v2, send_some_coins, UtxoFilter,
    ISSUER_MNEMONIC, OWNER_MNEMONIC,
};

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
