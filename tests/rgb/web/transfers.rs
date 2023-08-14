#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]
use std::collections::HashMap;
use std::{assert_eq, str::FromStr, vec};

use crate::rgb::web::utils::{new_block, send_coins};
use bdk::blockchain::EsploraBlockchain;
use bitcoin::{consensus, Transaction};
use bitmask_core::{
    debug, info,
    rgb::{prefetch::prefetch_resolver_txs, resolvers::ExplorerResolver},
    structs::{
        AssetType, BatchRgbTransferResponse, ContractResponse, ContractsResponse,
        DecryptedWalletData, FullRgbTransferRequest, FundVaultDetails, ImportRequest,
        InvoiceRequest, InvoiceResponse, IssueRequest, IssueResponse, NextAddressResponse,
        NextUtxoResponse, PsbtFeeRequest, RgbSaveTransferRequest, RgbTransferRequest,
        RgbTransferResponse, RgbTransferStatusResponse, SecretString, SignPsbtRequest,
        SignPsbtResponse, WalletData, WatcherRequest, WatcherResponse,
    },
    web::{
        bitcoin::{
            decrypt_wallet, encrypt_wallet, get_assets_vault, get_wallet_data, hash_password,
        },
        json_parse, resolve,
        rgb::{
            create_watcher, full_transfer_asset, import_contract, issue_contract, list_contracts,
            psbt_sign_file, rgb_create_invoice, save_transfer, verify_transfers,
            watcher_next_address, watcher_next_utxo,
        },
        set_panic_hook,
    },
};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::console;

wasm_bindgen_test_configure!(run_in_browser);

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[wasm_bindgen_test]
async fn create_contract_and_transfer() {
    set_panic_hook();
    let issuer_mnemonic =
        "ordinary crucial edit settle pencil lion appear unlock left fly century license";
    let owner_mnemonic =
        "apology pull visa moon retreat spell elite extend secret region fly diary";
    let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());

    info!("Import wallet");
    let issuer_mnemonic = resolve(encrypt_wallet(
        issuer_mnemonic.to_owned(),
        hash.clone(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    let issuer_mnemonic: SecretString = json_parse(&issuer_mnemonic);

    let owner_mnemonic = resolve(encrypt_wallet(
        owner_mnemonic.to_owned(),
        hash.clone(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    let owner_mnemonic: SecretString = json_parse(&owner_mnemonic);

    info!("Get Issuer Vault");
    let issuer_vault: JsValue =
        resolve(decrypt_wallet(hash.clone(), issuer_mnemonic.to_string())).await;
    let issuer_vault: DecryptedWalletData = json_parse(&issuer_vault);

    info!("Get Owner Vault");
    let owner_vault: JsValue = resolve(decrypt_wallet(hash, owner_mnemonic.to_string())).await;
    let owner_vault: DecryptedWalletData = json_parse(&owner_vault);

    info!("Create Issuer Watcher");
    let iface = "RGB20";
    let watcher_name = "default";
    let issuer_watcher_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_vault.public.watcher_xpub.clone(),
        force: true,
    };

    let issuer_sk = &issuer_vault.private.nostr_prv;
    let issuer_watcher_req = serde_wasm_bindgen::to_value(&issuer_watcher_req).expect("");
    let watcher_resp: JsValue = resolve(create_watcher(
        issuer_sk.clone(),
        issuer_watcher_req.clone(),
    ))
    .await;
    let watcher_resp: WatcherResponse = json_parse(&watcher_resp);

    info!("Get Address");
    let next_address: JsValue = resolve(watcher_next_address(
        issuer_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let next_address: NextAddressResponse = json_parse(&next_address);
    debug!(format!("Issuer Show Address {}", next_address.address));
    let _ = send_coins(&next_address.address, "1").await;

    info!("Create Owner Watcher");
    let owner_watcher_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_vault.public.watcher_xpub.clone(),
        force: true,
    };
    let owner_sk = &owner_vault.private.nostr_prv;
    let owner_watcher_req = serde_wasm_bindgen::to_value(&owner_watcher_req).expect("");
    let watcher_resp: JsValue = resolve(create_watcher(
        owner_sk.to_string(),
        owner_watcher_req.clone(),
    ))
    .await;
    let watcher_resp: WatcherResponse = json_parse(&watcher_resp);

    info!("Get Address");
    let next_address: JsValue = resolve(watcher_next_address(
        owner_sk.to_string(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let next_address: NextAddressResponse = json_parse(&next_address);
    debug!(format!("Owner Show Address {}", next_address.address));
    let _ = send_coins(&next_address.address, "1").await;

    info!("Get UTXO (Issuer)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        issuer_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let issuer_next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    debug!(format!("UTXO (Issuer): {:?}", issuer_next_utxo.utxo));

    info!("Get UTXO (Owner)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        owner_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let owner_next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    debug!(format!("UTXO (Owner): {:?}", owner_next_utxo.utxo));

    assert!(issuer_next_utxo.utxo.is_some());
    assert!(owner_next_utxo.utxo.is_some());

    info!("Create Contract (Issuer)");
    let supply = 100_000;
    let issue_utxo = issuer_next_utxo.utxo.unwrap().outpoint.to_string();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let issue_req = IssueRequest {
        ticker: "DIBA".to_string(),
        name: "DIBA".to_string(),
        description: "DIBA".to_string(),
        precision: 2,
        supply,
        seal: issue_seal.to_owned(),
        iface: iface.to_string(),
        meta: None,
    };

    let issue_req = serde_wasm_bindgen::to_value(&issue_req).expect("");
    let issue_resp: JsValue = resolve(issue_contract(issuer_sk.to_string(), issue_req)).await;
    let issuer_resp: IssueResponse = json_parse(&issue_resp);

    info!("::: SEND INVOICE FIRST TIME :::");
    info!("Create Invoice (Owner)");
    let params = HashMap::new();
    let owner_utxo = owner_next_utxo.utxo.unwrap().outpoint.to_string();
    let owner_seal = format!("tapret1st:{owner_utxo}");
    let invoice_req = InvoiceRequest {
        contract_id: issuer_resp.contract_id.to_string(),
        iface: issuer_resp.iface.to_string(),
        amount: 2000,
        seal: owner_seal,
        params,
    };
    let invoice_req = serde_wasm_bindgen::to_value(&invoice_req).expect("");
    let invoice_resp: JsValue =
        resolve(rgb_create_invoice(issuer_sk.to_string(), invoice_req)).await;
    let invoice_resp: InvoiceResponse = json_parse(&invoice_resp);

    info!("Create Payment (Issuer)");
    let issuer_desc = issuer_vault.public.rgb_assets_descriptor_xpub.to_string();
    let full_transfer_req = FullRgbTransferRequest {
        contract_id: issuer_resp.contract_id.to_string(),
        iface: issuer_resp.iface.to_string(),
        rgb_invoice: invoice_resp.invoice.to_string(),
        descriptor: SecretString(issuer_desc.to_string()),
        change_terminal: "/20/1".to_string(),
        fee: PsbtFeeRequest::Value(1000),
        bitcoin_changes: vec![],
    };

    let full_transfer_req = serde_wasm_bindgen::to_value(&full_transfer_req).expect("");
    let full_transfer_resp: JsValue = resolve(full_transfer_asset(
        issuer_sk.to_string(),
        full_transfer_req,
    ))
    .await;
    let full_transfer_resp: RgbTransferResponse = json_parse(&full_transfer_resp);
    debug!(format!(
        "Payment (Issuer): {:?}",
        full_transfer_resp.consig_id
    ));

    info!("Sign PSBT (Issuer)");
    let psbt_req = SignPsbtRequest {
        psbt: full_transfer_resp.psbt,
        descriptors: vec![
            SecretString(issuer_vault.private.rgb_assets_descriptor_xprv.clone()),
            SecretString(issuer_vault.private.btc_descriptor_xprv.clone()),
            SecretString(issuer_vault.private.btc_change_descriptor_xprv.clone()),
        ],
    };

    let psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
    let psbt_resp: JsValue = resolve(psbt_sign_file(issuer_sk.to_string(), psbt_req)).await;
    let psbt_resp: SignPsbtResponse = json_parse(&psbt_resp);
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    info!("Create new Block");
    let resp = new_block().await;
    debug!(format!("Block Created: {:?}", resp));

    info!("Save Consig (Owner)");
    let all_sks = [owner_sk.clone()];
    for sk in all_sks {
        let save_transfer_req = RgbSaveTransferRequest {
            contract_id: issuer_resp.contract_id.clone(),
            consignment: full_transfer_resp.consig.clone(),
        };
        let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");
        let save_transfer_resp = resolve(save_transfer(sk.to_string(), save_transfer_req)).await;
        let save_transfer_resp: RgbTransferStatusResponse = json_parse(&save_transfer_resp);
        debug!(format!("Save Consig: {:?}", save_transfer_resp));
    }

    info!("::: SEND INVOICE SECOND TIME :::");
    info!("Verify Consig (Both)");
    let all_sks = [owner_sk.clone(), issuer_sk.clone()];
    for sk in all_sks {
        let verify_transfer_resp = resolve(verify_transfers(sk.to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!("Verify Consig: {:?}", verify_transfer_resp));
    }

    info!("Create Invoice (Owner)");
    let params = HashMap::new();
    let owner_seal = format!("tapret1st:{owner_utxo}");
    let invoice_req = InvoiceRequest {
        contract_id: issuer_resp.contract_id.to_string(),
        iface: issuer_resp.iface.to_string(),
        amount: 3000,
        seal: owner_seal,
        params,
    };
    let invoice_req = serde_wasm_bindgen::to_value(&invoice_req).expect("");
    let invoice_resp: JsValue =
        resolve(rgb_create_invoice(issuer_sk.to_string(), invoice_req)).await;
    let invoice_resp: InvoiceResponse = json_parse(&invoice_resp);

    info!("Create Payment (Issuer)");
    let issuer_desc = issuer_vault.public.rgb_assets_descriptor_xpub.to_string();
    let full_transfer_req = FullRgbTransferRequest {
        contract_id: issuer_resp.contract_id.to_string(),
        iface: issuer_resp.iface.to_string(),
        rgb_invoice: invoice_resp.invoice.to_string(),
        descriptor: SecretString(issuer_desc.to_string()),
        change_terminal: "/20/1".to_string(),
        fee: PsbtFeeRequest::Value(1000),
        bitcoin_changes: vec![],
    };

    let full_transfer_req = serde_wasm_bindgen::to_value(&full_transfer_req).expect("");
    let full_transfer_resp: JsValue = resolve(full_transfer_asset(
        issuer_sk.to_string(),
        full_transfer_req,
    ))
    .await;
    let full_transfer_resp: RgbTransferResponse = json_parse(&full_transfer_resp);
    debug!(format!(
        "Payment (Issuer): {:?}",
        full_transfer_resp.consig_id
    ));

    info!("Sign PSBT (Issuer)");
    let psbt_req = SignPsbtRequest {
        psbt: full_transfer_resp.psbt,
        descriptors: vec![
            SecretString(issuer_vault.private.rgb_assets_descriptor_xprv.clone()),
            SecretString(issuer_vault.private.btc_descriptor_xprv.clone()),
            SecretString(issuer_vault.private.btc_change_descriptor_xprv.clone()),
        ],
    };

    let psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
    let psbt_resp: JsValue = resolve(psbt_sign_file(issuer_sk.to_string(), psbt_req)).await;
    let psbt_resp: SignPsbtResponse = json_parse(&psbt_resp);
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    info!("Create new Block");
    let resp = new_block().await;
    debug!(format!("Block Created: {:?}", resp));

    info!("Save Consig (Owner)");
    let all_sks = [owner_sk.clone()];
    for sk in all_sks {
        let save_transfer_req = RgbSaveTransferRequest {
            contract_id: issuer_resp.contract_id.clone(),
            consignment: full_transfer_resp.consig.clone(),
        };
        let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");
        let save_transfer_resp = resolve(save_transfer(sk.to_string(), save_transfer_req)).await;
        let save_transfer_resp: RgbTransferStatusResponse = json_parse(&save_transfer_resp);
        debug!(format!("Save Consig: {:?}", save_transfer_resp));
    }

    info!("Verify Consig (Both)");
    let all_sks = [owner_sk.clone(), issuer_sk.clone()];
    for sk in all_sks {
        let verify_transfer_resp = resolve(verify_transfers(sk.to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!("Verify Consig: {:?}", verify_transfer_resp));
    }
}
