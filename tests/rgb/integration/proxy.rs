#![cfg(not(target_arch = "wasm32"))]

use std::{collections::BTreeMap, str::FromStr};

use anyhow::Result;
use bitmask_core::{
    bitcoin::new_mnemonic,
    rgb::{
        create_watcher, get_contract,
        proxy::{pull_consignmnet, push_consignmnets},
        structs::ContractAmount,
        watcher_next_address,
    },
    structs::{RgbTransferResponse, SecretString, WatcherRequest},
};
use rgbwallet::RgbInvoice;

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt_v2, create_new_transfer, issuer_issue_contract_v2,
    send_some_coins, UtxoFilter,
};

#[tokio::test]
pub async fn store_and_retrieve_file_by_proxy() -> Result<()> {
    // 1. Initial Setup
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

    // 5. Store in RGB Proxy
    let RgbTransferResponse {
        consig: expected, ..
    } = transfer_resp;
    let rgb_invoice = RgbInvoice::from_str(&owner_resp.invoice)?;
    let consig_or_receipt_id = rgb_invoice.beneficiary.to_string();

    let mut consigs = BTreeMap::new();
    consigs.insert(consig_or_receipt_id.clone(), expected.clone());

    push_consignmnets(consigs).await?;

    // 6. Retrieve in RGB Proxy
    let consig = pull_consignmnet(consig_or_receipt_id).await?;
    assert_eq!(expected.to_string(), consig.unwrap_or_default().to_string());

    Ok(())
}
