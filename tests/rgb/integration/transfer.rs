#![cfg(not(target_arch = "wasm32"))]
use std::str::FromStr;

use bdk::wallet::AddressIndex;
use bitcoin::{consensus, psbt::PartiallySignedTransaction, Transaction};
use bitmask_core::{
    bitcoin::{get_wallet, save_mnemonic, sign_psbt, sign_psbt_file, synchronize_wallet},
    rgb::{
        accept_transfer, create_invoice, create_psbt, create_watcher, import, issue_contract,
        transfer_asset, watcher_details, watcher_next_address, watcher_next_utxo,
    },
    structs::{
        AcceptRequest, AllocationDetail, EncryptedWalletData, ImportRequest, ImportType,
        InvoiceRequest, InvoiceResponse, IssueRequest, IssueResponse, PsbtRequest, PsbtResponse,
        RgbTransferRequest, RgbTransferResponse, SignPsbtRequest, WatcherRequest,
    },
};
use futures::executor::block_on;
use psbt::Psbt;

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
    let issuer_resp = issuer_issue_contract(false).await?;
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let resp = create_new_psbt(issuer_keys, issuer_resp).await;
    assert!(resp.is_ok());

    Ok(())
}

#[tokio::test]
async fn allow_issuer_transfer_asset() -> anyhow::Result<()> {
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract(false).await?;
    let owner_resp = create_new_invoice(issuer_resp.clone()).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp).await?;
    let resp = create_new_transfer(issuer_keys, owner_resp, psbt_resp).await;
    assert!(resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_beneficiary_accept_tranfer() -> anyhow::Result<()> {
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract(false).await?;
    let owner_resp = create_new_invoice(issuer_resp.clone()).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp).await?;
    let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

    let sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        mnemonic: ISSUER_MNEMONIC.to_string(),
        seed_password: String::new(),
    };
    let resp = sign_psbt_file(&sk, request).await;
    assert!(resp.is_ok());

    let request = AcceptRequest {
        consignment: transfer_resp.consig,
        force: false,
    };

    let resp = accept_transfer(&sk, request).await;
    assert!(resp.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_issuer_sign_psbt() -> anyhow::Result<()> {
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let issuer_resp = issuer_issue_contract(false).await?;
    let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp).await?;

    let original_psbt = Psbt::from_str(&psbt_resp.psbt)?;
    let final_psbt = PartiallySignedTransaction::from(original_psbt);

    let issuer_wallet = get_wallet(&issuer_keys.private.rgb_assets_descriptor_xprv, None).await?;
    synchronize_wallet(&issuer_wallet).await?;

    let sign = sign_psbt(&issuer_wallet, final_psbt).await;
    assert!(sign.is_ok());
    Ok(())
}

async fn issuer_issue_contract(force: bool) -> Result<IssueResponse, anyhow::Error> {
    setup_regtest(force, None).await;
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let watcher_name = "default";
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.xpub.to_string(),
    };

    // Create Watcher
    let sk = issuer_keys.private.nostr_prv;
    let resp = create_watcher(&sk, create_watch_req).await;
    assert!(resp.is_ok());

    let next_address = watcher_next_address(&sk, watcher_name).await?;
    send_some_coins(&next_address.address, "0.1").await;

    let next_utxo = watcher_next_utxo(&sk, watcher_name).await?;

    let issue_utxo = next_utxo.utxo;
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

    let resp = issue_contract(&sk, request).await;
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

async fn create_new_psbt(
    issuer_keys: EncryptedWalletData,
    issuer_resp: IssueResponse,
) -> Result<PsbtResponse, anyhow::Error> {
    // Get Allocations
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv;
    let resp = watcher_details(&sk, watcher_name).await;
    assert!(resp.is_ok());

    let mut asset_utxo = String::new();
    let mut asset_utxo_terminal = String::new();
    let watcher_details = resp?;
    for contract_allocations in watcher_details.contracts {
        let allocations: Vec<AllocationDetail> = contract_allocations
            .allocations
            .into_iter()
            .filter(|a| a.is_mine && a.utxo == issuer_resp.issue_utxo)
            .collect();

        for allocation in allocations.into_iter() {
            asset_utxo = allocation.utxo.to_owned();
            asset_utxo_terminal = allocation.derivation.to_owned();
            break;
        }
    }

    assert_eq!(asset_utxo, issuer_resp.issue_utxo);
    let req = PsbtRequest {
        descriptor_pub: issuer_keys.public.rgb_assets_descriptor_xpub,
        asset_utxo: asset_utxo.to_string(),
        asset_utxo_terminal: asset_utxo_terminal.to_string(),
        change_index: None,
        bitcoin_changes: vec![],
        fee: 1000,
        input_tweak: None,
    };

    let resp = create_psbt(&sk, req).await;
    resp
}

async fn create_new_transfer(
    issuer_keys: EncryptedWalletData,
    owner_resp: InvoiceResponse,
    psbt_resp: PsbtResponse,
) -> Result<RgbTransferResponse, anyhow::Error> {
    // Get Allocations
    let transfer_req = RgbTransferRequest {
        psbt: psbt_resp.psbt,
        rgb_invoice: owner_resp.invoice,
    };

    let sk = issuer_keys.private.nostr_prv;
    let resp = transfer_asset(&sk, transfer_req).await;
    resp
}

#[tokio::test]
async fn test_blocking_server() -> anyhow::Result<()> {
    let txid = bitcoin::Txid::from_str(
        "6a64b7ed232f6d66409ad6716f51b5915ca999b3da356d924aae48dc7fcd3e04",
    )?;

    block_on(async {
        let final_url = &format!("{}/tx/{}/raw", "https://mempool.space/testnet/api", txid);
        let result = surf::get(final_url).recv_bytes().await.expect("");
        let tx: Transaction = consensus::deserialize::<Transaction>(&result).expect("");
        println!("{:?}", tx);
    });

    Ok(())
}
