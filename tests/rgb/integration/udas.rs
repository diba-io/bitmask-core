#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, get_uda_data, issuer_issue_contract,
    ISSUER_MNEMONIC,
};
use bitcoin::psbt::PartiallySignedTransaction;
use bitmask_core::{
    bitcoin::{get_wallet, save_mnemonic, sign_psbt, sign_psbt_file, synchronize_wallet},
    rgb::accept_transfer,
    structs::{AcceptRequest, SignPsbtRequest},
};
use psbt::Psbt;
use std::str::FromStr;

#[tokio::test]
async fn allow_beneficiary_create_invoice() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, single).await;
    let invoice_resp = create_new_invoice(issuer_resp?, None).await;
    assert!(invoice_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_create_psbt() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, single).await?;
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let resp = create_new_psbt(issuer_keys, issuer_resp).await;
    assert!(resp.is_ok());

    Ok(())
}

#[tokio::test]
async fn allow_issuer_transfer_asset() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, single).await?;
    let owner_resp = create_new_invoice(issuer_resp.clone(), None).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp).await?;
    let resp = create_new_transfer(issuer_keys, owner_resp, psbt_resp).await;
    assert!(resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_sign_psbt() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, single).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp.clone()).await?;

    let original_psbt = Psbt::from_str(&psbt_resp.psbt)?;
    let final_psbt = PartiallySignedTransaction::from(original_psbt);

    let descriptor_pub = match issuer_resp.iface.as_str() {
        "RGB20" => issuer_keys.private.rgb_assets_descriptor_xprv,
        "RGB21" => issuer_keys.private.rgb_udas_descriptor_xprv,
        _ => issuer_keys.public.rgb_assets_descriptor_xpub,
    };

    let issuer_wallet = get_wallet(&descriptor_pub, None).await?;
    synchronize_wallet(&issuer_wallet).await?;

    let sign = sign_psbt(&issuer_wallet, final_psbt).await;
    assert!(sign.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_beneficiary_accept_transfer() -> anyhow::Result<()> {
    let single = Some(get_uda_data());
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract("RGB21", 1, false, single).await?;
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
