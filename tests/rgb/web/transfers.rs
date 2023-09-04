#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]
use std::collections::HashMap;
use std::{assert_eq, str::FromStr, vec};

use crate::rgb::web::utils::{new_block, send_coins};
use bdk::blockchain::EsploraBlockchain;
use bitcoin::{consensus, Transaction};
use bitmask_core::web::constants::sleep;
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
            new_mnemonic,
        },
        json_parse, resolve,
        rgb::{
            create_watcher, full_transfer_asset, get_contract, import_contract, issue_contract,
            list_contracts, psbt_sign_file, rgb_create_invoice, save_transfer, verify_transfers,
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

pub struct TransferRounds {
    pub send_amount: u64,
    pub is_issuer_sender: bool,
}

impl TransferRounds {
    pub fn with(send_amount: u64, is_issuer_sender: bool) -> Self {
        TransferRounds {
            send_amount,
            is_issuer_sender,
        }
    }
}

#[wasm_bindgen_test]
#[allow(unused_assignments)]
async fn create_transfer_with_fee_value() {
    set_panic_hook();
    // let issuer_mnemonic =
    //     "try engine hurt mushroom adapt club boring diagram barely rail cable vicious tower boss hurt";
    // let owner_mnemonic =
    //     "rally ready surround evil grace autumn merry lunch husband infant forum wet possible thought drink";
    // let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());

    // info!("Import wallet");
    // let issuer_mnemonic = resolve(encrypt_wallet(
    //     issuer_mnemonic.to_owned(),
    //     hash.clone(),
    //     SEED_PASSWORD.to_owned(),
    // ))
    // .await;
    // let issuer_mnemonic: SecretString = json_parse(&issuer_mnemonic);

    // let owner_mnemonic = resolve(encrypt_wallet(
    //     owner_mnemonic.to_owned(),
    //     hash.clone(),
    //     SEED_PASSWORD.to_owned(),
    // ))
    // .await;
    // let owner_mnemonic: SecretString = json_parse(&owner_mnemonic);

    // info!("Get Issuer Vault");
    // let issuer_vault: JsValue =
    //     resolve(decrypt_wallet(hash.clone(), issuer_mnemonic.to_string())).await;
    // let issuer_vault: DecryptedWalletData = json_parse(&issuer_vault);

    // info!("Get Owner Vault");
    // let owner_vault: JsValue = resolve(decrypt_wallet(hash, owner_mnemonic.to_string())).await;
    // let owner_vault: DecryptedWalletData = json_parse(&owner_vault);

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

    let mut total_issuer = supply;
    let mut total_owner = 0;
    let rounds = vec![
        TransferRounds::with(20, true),
        TransferRounds::with(20, false),
        // TransferRounds::with(3_000, true),
        // TransferRounds::with(5_000, true),
        // TransferRounds::with(20, true),
        // TransferRounds::with(8_000, false),
        // TransferRounds::with(9_000, true),
        // TransferRounds::with(9_000, false),
        // TransferRounds::with(9_000, true),
        // TransferRounds::with(20, false),
        // TransferRounds::with(50_000, true),
    ];

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
        let invoice_req = InvoiceRequest {
            contract_id: issuer_resp.contract_id.to_string(),
            iface: issuer_resp.iface.to_string(),
            amount: round.send_amount,
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

        info!(format!("Sign PSBT ({sender})"));
        let psbt_req = SignPsbtRequest {
            psbt: full_transfer_resp.psbt,
            descriptors: sender_keys,
        };

        let psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
        let psbt_resp: JsValue = resolve(psbt_sign_file(sender_sk.to_string(), psbt_req)).await;
        let psbt_resp: SignPsbtResponse = json_parse(&psbt_resp);
        debug!(format!("Sign Psbt: {:?}", psbt_resp));

        info!("Create new Block");
        let resp = new_block().await;
        debug!(format!("Block Created: {:?}", resp));

        info!(format!("Save Consig ({receiver})"));
        let save_transfer_req = RgbSaveTransferRequest {
            iface: issuer_resp.iface.clone(),
            consignment: full_transfer_resp.consig.clone(),
        };
        let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");
        let save_transfer_resp =
            resolve(save_transfer(receiver_sk.to_string(), save_transfer_req)).await;
        let save_transfer_resp: RgbTransferStatusResponse = json_parse(&save_transfer_resp);
        debug!(format!("Save Consig: {:?}", save_transfer_resp));

        info!("Verify Consig (Both)");
        let verify_transfer_resp = resolve(verify_transfers(sender_sk.to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!(
            "Verify Consig ({sender}): {:?}",
            verify_transfer_resp
        ));

        let verify_transfer_resp = resolve(verify_transfers(receiver_sk.to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!(
            "Verify Consig ({receiver}): {:?}",
            verify_transfer_resp
        ));

        let (sender_balance, receiver_balance) = if round.is_issuer_sender {
            total_issuer -= round.send_amount;
            total_owner += round.send_amount;
            (total_issuer, total_owner)
        } else {
            total_issuer += round.send_amount;
            total_owner -= round.send_amount;
            (total_owner, total_issuer)
        };

        info!(format!("Get Contract Balancer ({sender})"));
        let contract_resp = resolve(get_contract(
            sender_sk.to_string(),
            issuer_resp.contract_id.clone(),
        ))
        .await;
        let contract_resp: ContractResponse = json_parse(&contract_resp);
        debug!(format!(
            "Contract ({sender}): {} ({})\n {:#?}",
            contract_resp.contract_id, contract_resp.balance, contract_resp.allocations
        ));
        let sender_current_balance = contract_resp.balance;

        info!(format!("Get Contract Balancer ({receiver})"));
        let contract_resp = resolve(get_contract(
            receiver_sk.to_string(),
            issuer_resp.contract_id.clone(),
        ))
        .await;
        let contract_resp: ContractResponse = json_parse(&contract_resp);
        debug!(format!(
            "Contract ({receiver}): {} ({})\n {:#?}",
            contract_resp.contract_id, contract_resp.balance, contract_resp.allocations
        ));
        let receiver_current_balance = contract_resp.balance;

        info!(format!("<<<< ROUND #{index} Finish >>>>"));
        assert_eq!(sender_current_balance, sender_balance);
        assert_eq!(receiver_current_balance, receiver_balance);
    }
}

#[wasm_bindgen_test]
#[allow(unused_assignments)]
async fn create_transfer_with_fee_rate() {
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
    let resp = send_coins(&issuer_next_address.address, "0.00010000").await;
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

    let mut total_issuer = supply;
    let mut total_owner = 0;
    let rounds = vec![TransferRounds::with(20, true)];

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
        let invoice_req = InvoiceRequest {
            contract_id: issuer_resp.contract_id.to_string(),
            iface: issuer_resp.iface.to_string(),
            amount: round.send_amount,
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
            fee: PsbtFeeRequest::FeeRate(1.1),
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

        info!(format!("Sign PSBT ({sender})"));
        let psbt_req = SignPsbtRequest {
            psbt: full_transfer_resp.psbt,
            descriptors: sender_keys,
        };

        let psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
        let psbt_resp: JsValue = resolve(psbt_sign_file(sender_sk.to_string(), psbt_req)).await;
        let psbt_resp: SignPsbtResponse = json_parse(&psbt_resp);
        debug!(format!("Sign Psbt: {:?}", psbt_resp));

        info!("Create new Block");
        let resp = new_block().await;
        debug!(format!("Block Created: {:?}", resp));

        info!(format!("Save Consig ({receiver})"));
        let save_transfer_req = RgbSaveTransferRequest {
            iface: issuer_resp.iface.clone(),
            consignment: full_transfer_resp.consig.clone(),
        };
        let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");
        let save_transfer_resp =
            resolve(save_transfer(receiver_sk.to_string(), save_transfer_req)).await;
        let save_transfer_resp: RgbTransferStatusResponse = json_parse(&save_transfer_resp);
        debug!(format!("Save Consig: {:?}", save_transfer_resp));

        info!("Verify Consig (Both)");
        let verify_transfer_resp = resolve(verify_transfers(sender_sk.to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!(
            "Verify Consig ({sender}): {:?}",
            verify_transfer_resp
        ));

        let verify_transfer_resp = resolve(verify_transfers(receiver_sk.to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!(
            "Verify Consig ({receiver}): {:?}",
            verify_transfer_resp
        ));

        let (sender_balance, receiver_balance) = if round.is_issuer_sender {
            total_issuer -= round.send_amount;
            total_owner += round.send_amount;
            (total_issuer, total_owner)
        } else {
            total_issuer += round.send_amount;
            total_owner -= round.send_amount;
            (total_owner, total_issuer)
        };

        info!(format!("Get Contract Balancer ({sender})"));
        let contract_resp = resolve(get_contract(
            sender_sk.to_string(),
            issuer_resp.contract_id.clone(),
        ))
        .await;
        let contract_resp: ContractResponse = json_parse(&contract_resp);
        debug!(format!(
            "Contract ({sender}): {} ({})\n {:#?}",
            contract_resp.contract_id, contract_resp.balance, contract_resp.allocations
        ));
        let sender_current_balance = contract_resp.balance;

        info!(format!("Get Contract Balancer ({receiver})"));
        let contract_resp = resolve(get_contract(
            receiver_sk.to_string(),
            issuer_resp.contract_id.clone(),
        ))
        .await;
        let contract_resp: ContractResponse = json_parse(&contract_resp);
        debug!(format!(
            "Contract ({receiver}): {} ({})\n {:#?}",
            contract_resp.contract_id, contract_resp.balance, contract_resp.allocations
        ));
        let receiver_current_balance = contract_resp.balance;

        info!(format!("<<<< ROUND #{index} Finish >>>>"));
        assert_eq!(sender_current_balance, sender_balance);
        assert_eq!(receiver_current_balance, receiver_balance);
    }
}
