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
        InvoiceRequest, InvoiceResponse, IssueRequest, IssueResponse, NextAddressResponse,
        NextUtxoResponse, PsbtFeeRequest, PublishedPsbtResponse, RgbBidRequest, RgbBidResponse,
        RgbOfferRequest, RgbOfferResponse, RgbSaveTransferRequest, RgbSwapRequest, RgbSwapResponse,
        RgbTransferRequest, RgbTransferResponse, RgbTransferStatusResponse, SecretString,
        SignPsbtRequest, SignedPsbtResponse, WalletData, WatcherRequest, WatcherResponse,
    },
    web::{
        bitcoin::{
            decrypt_wallet, encrypt_wallet, get_assets_vault, get_new_address, get_wallet_data,
            hash_password, new_mnemonic,
        },
        json_parse, resolve,
        rgb::{
            create_bid, create_offer, create_swap, create_watcher, full_transfer_asset,
            get_contract, import_contract, issue_contract, list_contracts, my_bids, my_offers,
            my_orders, psbt_sign_and_publish_file, psbt_sign_file, public_offers,
            rgb_create_invoice, save_transfer, verify_transfers, watcher_next_address,
            watcher_next_utxo,
        },
        set_panic_hook,
    },
};
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
    pub satoshi_price: u64,
    pub is_issuer_sender: bool,
}

impl TransferRounds {
    pub fn with(send_amount: f64, satoshi_price: u64, is_issuer_sender: bool) -> Self {
        TransferRounds {
            send_amount,
            satoshi_price,
            is_issuer_sender,
        }
    }
}

#[wasm_bindgen_test]
#[allow(unused_assignments)]
async fn create_transfer_swap_flow() {
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
    let btc_address_1 = resolve(get_new_address(
        issuer_vault.public.btc_descriptor_xpub.clone(),
        None,
    ))
    .await;
    let btc_address_1: String = json_parse(&btc_address_1);
    debug!(format!("Issuer Show Address {}", btc_address_1));

    let resp = send_coins(&btc_address_1, "1").await;
    debug!(format!("Issuer Receive Asset {:?}", resp));

    let asset_address_1 = resolve(get_new_address(
        issuer_vault.public.rgb_assets_descriptor_xpub.clone(),
        None,
    ))
    .await;
    let asset_address_1: String = json_parse(&asset_address_1);
    debug!(format!("Issuer Show Asset Address {}", asset_address_1));

    let resp = send_coins(&asset_address_1, "1").await;
    debug!(format!("Issuer Receive Asset Bitcoin {:?}", resp));

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
    let btc_address_1 = resolve(get_new_address(
        owner_vault.public.btc_descriptor_xpub.clone(),
        None,
    ))
    .await;
    let btc_address_1: String = json_parse(&btc_address_1);
    debug!(format!("Owner Show Address {}", btc_address_1));

    let resp = send_coins(&btc_address_1, "1").await;
    debug!(format!("Owner Receive Asset {:?}", resp));

    let asset_address_1 = resolve(get_new_address(
        owner_vault.public.rgb_assets_descriptor_xpub.clone(),
        None,
    ))
    .await;
    let asset_address_1: String = json_parse(&asset_address_1);
    debug!(format!("Owner Show Asset Address {}", asset_address_1));

    let resp = send_coins(&asset_address_1, "1").await;
    debug!(format!("Owner Receive Asset Bitcoin {:?}", resp));

    info!("Get UTXO (Owner)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        owner_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let owner_next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    debug!(format!("Owner UTXO {:?}", owner_next_utxo.utxo));

    info!("Get UTXO (Issuer)");
    let next_utxo: JsValue = resolve(watcher_next_utxo(
        issuer_sk.clone(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let issuer_next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    debug!(format!("Issuer UTXO {:?}", issuer_next_utxo.utxo));

    assert!(issuer_next_utxo.utxo.is_some());
    assert!(owner_next_utxo.utxo.is_some());

    info!("Create Contract (Issuer)");
    let supply = 500_000;
    let precision = 2;
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

    info!("Import Contract (Owner)");
    let contract_import = ImportRequest {
        import: AssetType::RGB20,
        data: issuer_resp.contract.strict,
    };

    let req = serde_wasm_bindgen::to_value(&contract_import).expect("oh no!");
    let resp = resolve(import_contract(owner_sk.clone(), req)).await;
    let resp: ContractResponse = json_parse(&resp);

    let mut total_issuer =
        f64::from_str(&ContractAmount::new(supply, precision).to_string()).unwrap();
    let mut total_owner = 0.0;
    let rounds = vec![TransferRounds::with(4_000.00, 1_000, true)];

    let mut sender = String::new();
    let mut sender_sk = String::new();
    let mut sender_desc = String::new();
    let mut sender_keys = vec![];

    let mut receiver = String::new();
    let mut receiver_sk = String::new();
    let mut receiver_desc = String::new();
    let mut receiver_keys = vec![];

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
            receiver_desc = owner_vault.public.rgb_assets_descriptor_xpub.to_string();
            receiver_keys = vec![
                SecretString(owner_vault.private.rgb_assets_descriptor_xprv.clone()),
                SecretString(owner_vault.private.btc_descriptor_xprv.clone()),
                SecretString(owner_vault.private.btc_change_descriptor_xprv.clone()),
            ];
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
            receiver_desc = issuer_vault.public.rgb_assets_descriptor_xpub.to_string();
            receiver_keys = vec![
                SecretString(issuer_vault.private.rgb_assets_descriptor_xprv.clone()),
                SecretString(issuer_vault.private.btc_descriptor_xprv.clone()),
                SecretString(issuer_vault.private.btc_change_descriptor_xprv.clone()),
            ];
        }

        info!(format!(
            ">>>> ROUND #{index} {sender} SWAP {} units to {receiver} <<<<",
            round.send_amount
        ));

        info!(format!("Sender ({sender}) Create Offer"));
        let expire_at = (chrono::Local::now() + chrono::Duration::minutes(5))
            .naive_utc()
            .timestamp();
        let sender_asset_desc = sender_desc.clone();
        let asset_amount = ContractAmount::with(round.send_amount as u64, 0, 2);
        let sender_swap_req = RgbOfferRequest {
            contract_id: issuer_resp.contract_id.clone(),
            iface: issuer_resp.iface.clone(),
            contract_amount: asset_amount.to_value().to_string(),
            bitcoin_price: round.satoshi_price,
            descriptor: SecretString(sender_asset_desc),
            change_terminal: "/20/1".to_string(),
            bitcoin_changes: vec![],
            expire_at: Some(expire_at),
            presig: false,
        };
        let sender_swap_req = serde_wasm_bindgen::to_value(&sender_swap_req).expect("");

        let sender_swap_resp: JsValue =
            resolve(create_offer(sender_sk.clone(), sender_swap_req)).await;
        let sender_swap_resp: RgbOfferResponse = json_parse(&sender_swap_resp);

        info!(format!("Receiver ({receiver}) Create Bid"));
        let receiver_btc_desc = receiver_desc.clone();
        let asset_amount = ContractAmount::with(round.send_amount as u64, 0, 2);
        let receiver_swap_req = RgbBidRequest {
            offer_id: sender_swap_resp.offer_id.clone(),
            asset_amount: asset_amount.to_value().to_string(),
            descriptor: SecretString(receiver_btc_desc),
            change_terminal: "/1/0".to_string(),
            fee: PsbtFeeRequest::Value(1000),
        };
        let receiver_swap_req = serde_wasm_bindgen::to_value(&receiver_swap_req).expect("");

        let receiver_swap_resp: JsValue =
            resolve(create_bid(receiver_sk.clone(), receiver_swap_req)).await;
        let receiver_swap_resp: RgbBidResponse = json_parse(&receiver_swap_resp);

        info!(format!("Receiver ({receiver}) Sign Bid"));
        let psbt_req = SignPsbtRequest {
            psbt: receiver_swap_resp.swap_psbt,
            descriptors: receiver_keys.clone(),
        };

        let receiver_psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
        let receiver_psbt_resp: JsValue =
            resolve(psbt_sign_file(receiver_sk.clone(), receiver_psbt_req)).await;
        let receiver_psbt_resp: SignedPsbtResponse = json_parse(&receiver_psbt_resp);
        debug!(format!("Sign Bid Psbt: {}", receiver_psbt_resp.sign));

        info!(format!("Sender ({sender}) Create Swap"));
        let final_swap_req = RgbSwapRequest {
            offer_id: sender_swap_resp.offer_id.clone(),
            bid_id: receiver_swap_resp.bid_id,
            swap_psbt: receiver_psbt_resp.psbt,
        };
        let final_swap_req = serde_wasm_bindgen::to_value(&final_swap_req).expect("");

        let final_swap_res: JsValue = resolve(create_swap(sender_sk.clone(), final_swap_req)).await;
        let final_swap_res: RgbSwapResponse = json_parse(&final_swap_res);

        info!(format!("Sender ({sender}) Sign Swap"));
        let psbt_req = SignPsbtRequest {
            psbt: final_swap_res.final_psbt,
            descriptors: sender_keys,
        };

        let swap_psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
        let swap_psbt_resp: JsValue =
            resolve(psbt_sign_and_publish_file(sender_sk.clone(), swap_psbt_req)).await;
        let swap_psbt_resp: PublishedPsbtResponse = json_parse(&swap_psbt_resp);
        debug!(format!("Sign & Publish Psbt: {:?}", swap_psbt_resp));

        info!("Create new Block");
        let resp = new_block().await;
        debug!(format!("Block Created: {:?}", resp));

        info!(format!("Save Consig ({receiver})"));
        let save_transfer_req = RgbSaveTransferRequest {
            iface: issuer_resp.iface.clone(),
            consignment: final_swap_res.final_consig.clone(),
        };
        let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");
        let save_transfer_resp =
            resolve(save_transfer(receiver_sk.to_string(), save_transfer_req)).await;
        let save_transfer_resp: RgbTransferStatusResponse = json_parse(&save_transfer_resp);
        debug!(format!("Save Consig: {:?}", save_transfer_resp));

        info!("Verify Consig (Both)");
        let verify_transfer_resp = resolve(verify_transfers(sender_sk.clone().to_string())).await;
        let verify_transfer_resp: BatchRgbTransferResponse = json_parse(&verify_transfer_resp);
        debug!(format!(
            "Verify Consig ({sender}): {:?}",
            verify_transfer_resp
        ));

        let verify_transfer_resp = resolve(verify_transfers(receiver_sk.clone())).await;
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
        let sender_current_balance = contract_resp.balance_normalized;

        info!(format!("Get Contract Balancer ({receiver})"));
        let contract_resp =
            resolve(get_contract(receiver_sk, issuer_resp.contract_id.clone())).await;
        let contract_resp: ContractResponse = json_parse(&contract_resp);
        debug!(format!(
            "Contract ({receiver}): {} ({})\n {:#?}",
            contract_resp.contract_id, contract_resp.balance, contract_resp.allocations
        ));
        let receiver_current_balance = contract_resp.balance_normalized;

        info!(format!("<<<< ROUND #{index} Finish >>>>"));
        assert_eq!(sender_current_balance, sender_balance);
        assert_eq!(receiver_current_balance, receiver_balance);
    }
}
