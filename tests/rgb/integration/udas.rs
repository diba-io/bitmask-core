#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, get_uda_data,
    issuer_issue_contract_v2, UtxoFilter, ISSUER_MNEMONIC, OWNER_MNEMONIC,
};
use bitmask_core::{
    rgb::accept_transfer,
    save_mnemonic, sign_psbt_file,
    structs::{AcceptRequest, SecretString, SignPsbtRequest},
};

#[tokio::test]
async fn allow_beneficiary_accept_transfer() -> anyhow::Result<()> {
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
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = create_new_transfer(owner_resp, psbt_resp).await?;

    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        descriptors: [issuer_keys.private.rgb_udas_descriptor_xprv.clone()].to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    let request = AcceptRequest {
        consignment: transfer_resp.consig,
        force: false,
    };

    let resp = accept_transfer(request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    Ok(())
}
