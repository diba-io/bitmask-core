#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, get_uda_data, issuer_issue_contract,
    ISSUER_MNEMONIC, OWNER_MNEMONIC,
};
use bitmask_core::{
    bitcoin::{save_mnemonic, sign_psbt_file},
    rgb::accept_transfer,
    structs::{AcceptRequest, SecretString, SignPsbtRequest},
};

#[tokio::test]
async fn allow_beneficiary_accept_transfer() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_resp = &issuer_issue_contract("RGB21", 1, false, true, single).await?;
    let owner_resp = create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1,
        owner_keys,
        None,
        Some(issuer_resp.clone().contract.legacy),
    )
    .await?;
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        &issuer_resp.issue_utxo,
        issuer_keys.clone(),
        None,
    )
    .await?;
    let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

    let sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        descriptor: SecretString(issuer_keys.private.rgb_udas_descriptor_xprv.clone()),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    let request = AcceptRequest {
        consignment: transfer_resp.consig,
        force: false,
    };

    let resp = accept_transfer(&sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    Ok(())
}
