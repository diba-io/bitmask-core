#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]
use bitmask_core::{
    info,
    structs::{
        ContractResponse, ContractsResponse, DecryptedWalletData, IssueRequest,
        NextAddressResponse, NextUtxoResponse, SecretString, WatcherRequest, WatcherResponse,
    },
    web::{
        bitcoin::{
            decrypt_wallet, encrypt_wallet, get_assets_vault, get_wallet_data, hash_password,
        },
        constants::switch_network,
        json_parse, resolve,
        rgb::{
            create_watcher, get_contract_state, import_contract, issue_contract,
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

const ENCRYPTION_PASSWORD: &str = "";
const SEED_PASSWORD: &str = "";

// #[wasm_bindgen_test]
async fn inspect_contract_states() {
    set_panic_hook();
    let mnemonic = "";
    let hash = hash_password(ENCRYPTION_PASSWORD.to_owned());

    resolve(switch_network("bitcoin".to_string())).await;

    info!("Import Seed");
    let mnemonic_data_str = resolve(encrypt_wallet(
        mnemonic.to_owned(),
        hash.clone(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;
    let mnemonic_data: SecretString = json_parse(&mnemonic_data_str);

    info!("Get Vault");
    let wallet_keys: JsValue = resolve(decrypt_wallet(hash, mnemonic_data.0.clone())).await;

    info!("Get Keys");
    let wallet_keys: DecryptedWalletData = json_parse(&wallet_keys);
    let sk = &wallet_keys.private.nostr_prv;

    info!("Get Contract");
    let contract_id = "";
    let get_contract_resp: JsValue =
        resolve(get_contract_state(sk.to_string(), contract_id.to_string())).await;
    let get_contract_resp: ContractResponse = json_parse(&get_contract_resp);
    info!(format!(
        "Contract {} ({}): \n {:#?}",
        get_contract_resp.name, get_contract_resp.balance, get_contract_resp.allocations
    ));
}
