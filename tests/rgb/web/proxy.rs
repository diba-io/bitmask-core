#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]
use crate::rgb::web::utils::{new_block, send_coins};
use bdk::blockchain::EsploraBlockchain;
use bitcoin::{consensus, Transaction};
use bitmask_core::rgb::structs::ContractAmount;
use bitmask_core::web::constants::sleep;
use bitmask_core::{
    debug, info,
    rgb::{prefetch::prefetch_resolver_txs, resolvers::ExplorerResolver},
    structs::{
        AssetType, BatchRgbTransferResponse, ContractResponse, ContractsResponse,
        DecryptedWalletData, FullRgbTransferRequest, FundVaultDetails, ImportRequest,
        InvoiceRequest, InvoiceResponse, IssueMediaRequest, IssuePreRequest, IssueResponse,
        MediaItemRequest, MediaRequest, MediaResponse, NextAddressResponse, NextUtxoResponse,
        PsbtFeeRequest, PublishedPsbtResponse, RgbSaveTransferRequest, RgbTransferRequest,
        RgbTransferResponse, RgbTransferStatusResponse, SecretString, SignPsbtRequest, WalletData,
        WatcherRequest, WatcherResponse,
    },
    web::{
        bitcoin::{
            decrypt_wallet, encrypt_wallet, get_assets_vault, get_wallet_data, hash_password,
            new_mnemonic,
        },
        json_parse, resolve,
        rgb::{
            create_watcher, full_transfer_asset, get_consignment, get_contract,
            import_consignments, import_contract, import_uda_data, issue_contract, list_contracts,
            psbt_sign_and_publish_file, rgb_create_invoice, save_transfer, verify_transfers,
            watcher_next_address, watcher_next_utxo,
        },
        set_panic_hook,
    },
};
use rgbwallet::RgbInvoice;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::{assert_eq, str::FromStr, vec};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::console;

wasm_bindgen_test_configure!(run_in_browser);

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

pub struct TransferRounds {
    pub send_amount: f64,
    pub is_issuer_sender: bool,
}

impl TransferRounds {
    pub fn with(send_amount: f64, is_issuer_sender: bool) -> Self {
        TransferRounds {
            send_amount,
            is_issuer_sender,
        }
    }
}

#[wasm_bindgen_test]
#[allow(unused_assignments)]
async fn import_and_get_consig_from_proxy() {
    set_panic_hook();
    let issuer_vault = resolve(new_mnemonic("".to_string())).await;
    let issuer_vault: DecryptedWalletData = json_parse(&issuer_vault);
    let owner_vault = resolve(new_mnemonic("".to_string())).await;
    let owner_vault: DecryptedWalletData = json_parse(&owner_vault);

    info!("Create Issuer Watcher");
    let iface = "RGB20";
    let watcher_name = "default";
    let issuer_watcher_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_vault.public.watcher_xpub.clone(),
        force: false,
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
    let issuer_next_address: NextAddressResponse = json_parse(&next_address);
    debug!(format!(
        "Issuer Show Address {}",
        issuer_next_address.address
    ));
    let resp = send_coins(&issuer_next_address.address, "1").await;
    debug!(format!("Issuer Receive Bitcoin {:?}", resp));

    info!("Create Owner Watcher");
    let owner_watcher_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_vault.public.watcher_xpub.clone(),
        force: false,
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
    let owner_next_address: JsValue = resolve(watcher_next_address(
        owner_sk.to_string(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let owner_next_address: NextAddressResponse = json_parse(&owner_next_address);
    debug!(format!("Owner Show Address {}", owner_next_address.address));
    let resp = send_coins(&owner_next_address.address, "1").await;
    debug!(format!("Owner Receive Bitcoin {:?}", resp));

    info!("Get UTXO (Owner)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        owner_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let owner_next_utxo: NextUtxoResponse = json_parse(&next_utxo);

    info!("Get UTXO (Issuer)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        issuer_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let issuer_next_utxo: NextUtxoResponse = json_parse(&next_utxo);

    assert!(issuer_next_utxo.utxo.is_some());
    assert!(owner_next_utxo.utxo.is_some());

    info!("Create Contract (Issuer)");
    let supply = 100_000;
    let precision = 2;
    let issue_utxo = issuer_next_utxo.utxo.unwrap().outpoint.to_string();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let issue_req = IssuePreRequest {
        ticker: "DIBA".to_string(),
        name: "DIBA".to_string(),
        description: "DIBA".to_string(),
        precision,
        supply,
        seal: issue_seal.to_owned(),
        iface: iface.to_string(),
        meta: None,
    };

    let issue_req = serde_wasm_bindgen::to_value(&issue_req).expect("");
    let issue_resp: JsValue = resolve(issue_contract(issuer_sk.to_string(), issue_req)).await;
    let issuer_resp: IssueResponse = json_parse(&issue_resp);

    info!("Import Contract (Owner)");
    let contract_import = ImportRequest {
        import: AssetType::RGB20,
        data: issuer_resp.contract.strict,
    };

    let req = serde_wasm_bindgen::to_value(&contract_import).expect("oh no!");
    let resp = resolve(import_contract(owner_sk.clone(), req)).await;
    let resp: ContractResponse = json_parse(&resp);

    let rounds = vec![TransferRounds::with(20.00, true)];

    let mut sender = String::new();
    let mut sender_sk = String::new();
    let mut sender_desc = String::new();
    let mut sender_keys = vec![];

    let mut receiver = String::new();
    let mut receiver_sk = String::new();
    for (index, round) in rounds.into_iter().enumerate() {
        if round.is_issuer_sender {
            sender = "ISSUER".to_string();
            sender_sk = issuer_sk.to_string();
            sender_desc = issuer_vault.public.rgb_assets_descriptor_xpub.to_string();
            sender_keys = vec![
                SecretString(issuer_vault.private.rgb_assets_descriptor_xprv.clone()),
                SecretString(issuer_vault.private.btc_descriptor_xprv.clone()),
                SecretString(issuer_vault.private.btc_change_descriptor_xprv.clone()),
            ];

            receiver = "OWNER".to_string();
            receiver_sk = owner_sk.to_string();
        } else {
            sender = "OWNER".to_string();
            sender_sk = owner_sk.to_string();
            sender_desc = owner_vault.public.rgb_assets_descriptor_xpub.to_string();
            sender_keys = vec![
                SecretString(owner_vault.private.rgb_assets_descriptor_xprv.clone()),
                SecretString(owner_vault.private.btc_descriptor_xprv.clone()),
                SecretString(owner_vault.private.btc_change_descriptor_xprv.clone()),
            ];

            receiver = "ISSUER".to_string();
            receiver_sk = issuer_sk.to_string();
        }

        info!(format!(
            ">>>> ROUND #{index} {sender} SEND {} units to {receiver} <<<<",
            round.send_amount
        ));
        info!(format!("Get Receiver Next UTXO ({receiver})"));
        let next_utxo: JsValue = resolve(watcher_next_utxo(
            receiver_sk.clone(),
            watcher_name.to_string(),
            iface.to_string(),
        ))
        .await;
        let receiver_next_utxo: NextUtxoResponse = json_parse(&next_utxo);
        debug!(format!("UTXO ({receiver}): {:?}", receiver_next_utxo.utxo));

        info!(format!("Create Invoice ({receiver})"));
        let params = HashMap::new();
        let receiver_utxo = receiver_next_utxo.utxo.unwrap().outpoint.to_string();
        let receiver_seal = format!("tapret1st:{receiver_utxo}");
        let invoice_amount = ContractAmount::from(round.send_amount.to_string(), precision);
        let invoice_req = InvoiceRequest {
            contract_id: issuer_resp.contract_id.to_string(),
            iface: issuer_resp.iface.to_string(),
            amount: invoice_amount.to_string(),
            seal: receiver_seal,
            params,
        };

        let invoice_req = serde_wasm_bindgen::to_value(&invoice_req).expect("");
        let invoice_resp: JsValue =
            resolve(rgb_create_invoice(receiver_sk.to_string(), invoice_req)).await;
        let invoice_resp: InvoiceResponse = json_parse(&invoice_resp);
        debug!(format!(
            "Invoice ({receiver}): {}",
            invoice_resp.invoice.to_string()
        ));

        info!(format!("Create Payment ({sender})"));
        let full_transfer_req = FullRgbTransferRequest {
            contract_id: issuer_resp.contract_id.to_string(),
            iface: issuer_resp.iface.to_string(),
            rgb_invoice: invoice_resp.invoice.to_string(),
            descriptor: SecretString(sender_desc.to_string()),
            change_terminal: "/20/1".to_string(),
            fee: PsbtFeeRequest::Value(1000),
            bitcoin_changes: vec![],
        };

        let full_transfer_req = serde_wasm_bindgen::to_value(&full_transfer_req).expect("");
        let full_transfer_resp: JsValue = resolve(full_transfer_asset(
            sender_sk.to_string(),
            full_transfer_req,
        ))
        .await;
        let full_transfer_resp: RgbTransferResponse = json_parse(&full_transfer_resp);
        debug!(format!(
            "Payment ({sender}): {:?}",
            full_transfer_resp.consig_id
        ));

        info!(format!("Store Payment ({sender})"));
        let RgbTransferResponse {
            consig: expected, ..
        } = full_transfer_resp;

        let rgb_invoice = RgbInvoice::from_str(&invoice_resp.invoice).expect("invalid invoice");
        let consig_or_receipt_id = rgb_invoice.beneficiary.to_string();

        let mut consigs = BTreeMap::new();
        consigs.insert(consig_or_receipt_id.clone(), expected.clone());

        let consigs = serde_wasm_bindgen::to_value(&consigs).expect("");
        let import_req = resolve(import_consignments(consigs)).await;
        let full_transfer_resp: bool = json_parse(&import_req);

        info!(format!("Retrieve Payment ({sender})"));
        let get_req = resolve(get_consignment(consig_or_receipt_id)).await;
        let get_resp: Option<String> = json_parse(&get_req);
        let get_resp = get_resp.unwrap_or_default();

        assert_eq!(expected.to_string(), get_resp.to_string());
    }
}

#[wasm_bindgen_test]
#[allow(unused_assignments)]
async fn create_uda_with_medias() {
    set_panic_hook();
    let issuer_vault = resolve(new_mnemonic("".to_string())).await;
    let issuer_vault: DecryptedWalletData = json_parse(&issuer_vault);

    info!("Create Issuer Watcher");
    let iface = "RGB21";
    let watcher_name = "default";
    let issuer_watcher_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_vault.public.watcher_xpub.clone(),
        force: false,
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
    let issuer_next_address: NextAddressResponse = json_parse(&next_address);
    debug!(format!(
        "Issuer Show Address {}",
        issuer_next_address.address
    ));
    let resp = send_coins(&issuer_next_address.address, "1").await;
    debug!(format!("Issuer Receive Bitcoin {:?}", resp));

    info!("Get UTXO (Issuer)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        issuer_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let issuer_next_utxo: NextUtxoResponse = json_parse(&next_utxo);

    assert!(issuer_next_utxo.utxo.is_some());

    info!("Create Contract (Issuer)");
    let media_item = MediaItemRequest {
        ty: "image/svg+xml".to_string(),
        uri: "https://bitcoin.org/img/icons/logotop.svg".to_string(),
    };
    let import_media_req = MediaRequest {
        preview: Some(media_item.clone()),
        media: Some(media_item),
        attachments: vec![],
    };

    let supply = 1;
    let precision = 0;
    let issue_utxo = issuer_next_utxo.utxo.unwrap().outpoint.to_string();
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let issue_req = IssuePreRequest {
        ticker: "DIBA".to_string(),
        name: "DIBA".to_string(),
        description: "DIBA".to_string(),
        precision,
        supply,
        seal: issue_seal.to_owned(),
        iface: iface.to_string(),
        meta: Some(import_media_req),
    };

    let issue_req = serde_wasm_bindgen::to_value(&issue_req).expect("");
    let issue_resp: JsValue = resolve(issue_contract(issuer_sk.to_string(), issue_req)).await;
    let issuer_resp: IssueResponse = json_parse(&issue_resp);
}
