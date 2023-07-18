#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt_v2, create_new_transfer, issuer_issue_contract_v2,
    send_some_coins, UtxoFilter, ISSUER_MNEMONIC, OWNER_MNEMONIC,
};
use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, save_mnemonic, sign_psbt_file, sync_wallet},
    rgb::{accept_transfer, get_contract},
    structs::{AcceptRequest, PsbtInputRequest, SecretString, SignPsbtRequest},
};

#[tokio::test]
async fn allow_multiple_inputs_in_same_psbt() -> anyhow::Result<()> {
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
        5,
        false,
        true,
        None,
        Some("0.00001".to_string()),
        Some(UtxoFilter {
            outpoint_equal: None,
            amount_less_than: Some(1000),
        }),
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
        .get_address(AddressIndex::Peek(0))?
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
    let resp = sign_psbt_file(request).await;
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
