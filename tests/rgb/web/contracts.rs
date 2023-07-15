#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(all(target_arch = "wasm32"))]
use bitmask_core::{
    info,
    structs::{
        ContractsResponse, DecryptedWalletData, IssueRequest, NextAddressResponse,
        NextUtxoResponse, SecretString, WatcherRequest, WatcherResponse,
    },
    web::{
        bitcoin::{
            decrypt_wallet, encrypt_wallet, get_assets_vault, get_wallet_data, hash_password,
        },
        json_parse, resolve,
        rgb::{
            create_watcher, import_contract, issue_contract, list_contracts, watcher_next_address,
            watcher_next_utxo,
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

// #[wasm_bindgen_test]
async fn allow_issue_and_list_contracts() {
    set_panic_hook();
    let mnemonic = env!("TEST_WALLET_SEED", "TEST_WALLET_SEED variable not set");
    let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());

    info!("Import Seed");
    let mnemonic_data_str = resolve(encrypt_wallet(
        mnemonic.to_owned(),
        hash.clone(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    let mnemonic_data: SecretString = json_parse(&mnemonic_data_str);

    info!("Get Vault");
    let issuer_keys: JsValue = resolve(decrypt_wallet(hash, mnemonic_data.0.clone())).await;

    info!("Get Keys");
    let issuer_keys: DecryptedWalletData = json_parse(&issuer_keys);

    info!("Issue Contract");
    let sk = &issuer_keys.private.nostr_prv;

    info!("Create Watcher");
    let iface = "RGB20";
    let watcher_name = "default";
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };

    let create_watch_req = serde_wasm_bindgen::to_value(&create_watch_req).expect("");
    let watcher_resp: JsValue =
        resolve(create_watcher(sk.to_string(), create_watch_req.clone())).await;
    let watcher_resp: WatcherResponse = json_parse(&watcher_resp);

    info!("Get Address");
    let next_address: JsValue = resolve(watcher_next_address(
        sk.to_string(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let next_address: NextAddressResponse = json_parse(&next_address);
    info!(format!("Show Address {}", next_address.address));

    info!("Get UTXO");
    let next_address: JsValue = resolve(watcher_next_utxo(
        sk.to_string(),
        watcher_name.to_string(),
        iface.to_string(),
    ))
    .await;
    let next_utxo: NextUtxoResponse = json_parse(&next_address);
    let next_utxo = next_utxo.utxo.unwrap().outpoint;
    info!(format!("Show Utxo {}", next_utxo));

    info!("Generate Issue");
    let supply = 5;
    let issue_utxo = next_utxo;
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
    let issue_resp: JsValue = resolve(issue_contract(sk.to_string(), issue_req)).await;

    info!("List Contracts");
    let list_contracts_resp: JsValue = resolve(list_contracts(sk.to_string())).await;
    let list_contracts_resp: ContractsResponse = json_parse(&list_contracts_resp);
    info!(format!("Show Contracts {:?}", list_contracts_resp));
}
