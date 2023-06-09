#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, get_collectible_data,
    issuer_issue_contract, ISSUER_MNEMONIC,
};
use bitmask_core::{
    bitcoin::{save_mnemonic, sign_psbt_file},
    rgb::accept_transfer,
    structs::{AcceptRequest, SignPsbtRequest},
};

#[tokio::test]
async fn allow_beneficiary_accept_transfer() -> anyhow::Result<()> {
    let collectible = Some(get_collectible_data());
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, true, collectible).await?;
    let owner_resp = create_new_invoice(issuer_resp.clone(), None).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp.clone()).await?;
    let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

    let sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        mnemonic: ISSUER_MNEMONIC.to_string(),
        seed_password: String::new(),
        iface: issuer_resp.iface,
    };
    let resp = sign_psbt_file(&sk, request).await;
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
