#![cfg(not(target_arch = "wasm32"))]
use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, get_wallet_data, save_mnemonic, synchronize_wallet},
    rgb::{create_invoice, import, issue_contract},
    structs::{
        ImportRequest, ImportType, InvoiceRequest, InvoiceResponse, IssueRequest, IssueResponse,
        PsbtRequest,
    },
};

use super::utils::ISSUER_MNEMONIC;
use crate::rgb::integration::utils::{send_some_coins, setup_regtest, OWNER_MNEMONIC};

#[tokio::test]
/*
 * Issuer to Beneficiary
 */
async fn allow_issuer_issue_contract() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract(false).await;
    assert!(issuer_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_beneficiary_create_invoice() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract(false).await;
    let invoice_resp = create_new_invoice(issuer_resp?).await;
    assert!(invoice_resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_create_psbt() -> anyhow::Result<()> {
    let issuer_resp = issuer_issue_contract(false).await;
    let invoice_resp = create_new_invoice(issuer_resp?).await;

    // let req = PsbtRequest {};
    Ok(())
}

#[tokio::test]
async fn allow_issuer_transfer_asset() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_beneficiary_accept_tranfer() -> anyhow::Result<()> {
    Ok(())
}

async fn issuer_issue_contract(force: bool) -> Result<IssueResponse, anyhow::Error> {
    let issuer_vault = setup_regtest(force, Some(ISSUER_MNEMONIC)).await?;
    let fungible_wallet =
        get_wallet_data(&issuer_vault.public.rgb_assets_descriptor_xpub, None).await?;

    let issue_utxo = fungible_wallet.utxos.first().unwrap();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let request = IssueRequest {
        ticker: "DIBA1".to_string(),
        name: "DIBA1".to_string(),
        description: "DIBA1".to_string(),
        precision: 2,
        supply: 5,
        seal: issue_seal.to_owned(),
        iface: "RGB20".to_string(),
    };

    let nostr_sk = issuer_vault.private.nostr_prv.to_string();
    let resp = issue_contract(&nostr_sk, request).await;
    resp
}

async fn create_new_invoice(issuer_resp: IssueResponse) -> Result<InvoiceResponse, anyhow::Error> {
    let owner_data = save_mnemonic(OWNER_MNEMONIC, "").await?;
    let owner_vault = get_wallet(&owner_data.public.rgb_assets_descriptor_xpub, None).await?;

    // Import Contract
    let import_req = ImportRequest {
        import: ImportType::Contract,
        data: issuer_resp.contract,
    };
    let nostr_sk = owner_data.private.nostr_prv.to_string();
    let resp = import(&nostr_sk, import_req).await;
    assert!(resp.is_ok());

    // Create Invoice
    let owner_address = &owner_vault
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(owner_address, "0.1").await;
    synchronize_wallet(&owner_vault).await?;

    let beneficiary_utxo = owner_vault.list_unspent()?;
    let beneficiary_utxo = beneficiary_utxo.first().unwrap();
    let seal = beneficiary_utxo.outpoint.to_string();
    let seal = format!("tapret1st:{seal}");

    let invoice_req = InvoiceRequest {
        contract_id: issuer_resp.contract_id,
        iface: issuer_resp.iface,
        amount: 1,
        seal,
    };

    let resp = create_invoice(&nostr_sk, invoice_req).await;
    resp
}
