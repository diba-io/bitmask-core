#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use bitmask_core::{
    get_vault, get_wallet_data, import_asset, json_parse, resolve, save_mnemonic_seed,
    set_panic_hook, VaultData, WalletData,
};

wasm_bindgen_test_configure!(run_in_browser);

const MNEMONIC: &str = "then kidney town pair iron agent assault put oven erosion like govern";
const ENCRYPTION_PASSWORD: &str = "hunter2021";
const SEED_PASSWORD: &str = "";
const ENCRYPTED_DESCRIPTORS: &str = "[191,202,70,213,53,65,209,112,253,117,142,74,5,40,53,165,141,95,117,199,80,255,249,232,44,72,203,164,72,56,135,173,30,223,86,223,16,3,197,247,28,160,179,85,131,209,96,123,207,10,9,21,211,162,184,135,115,96,102,155,76,121,141,128,250,244,18,33,87,6,32,235,80,247,22,80,157,103,7,109,170,177,30,8,121,189,158,110,161,209,163,243,161,191,95,182,27,197,80,125,242,204,145,87,45,173,27,252,210,48,181,218,136,253,128,210,19,146,255,144,22,31,113,45,51,158,194,47,215,12,74,176,79,210,42,132,241,27,197,193,154,16,100,246,21,138,47,95,75,109,48,131,253,210,189,205,120,130,55,156,94,146,125,163,199,22,1,255,196,55,57,1,12,60,199,35,83,29,7,33,154,144,71,94,94,56,141,40,176,127,20,32,234,184,108,167,206,98,24,199,38,71,213,169,23,151,231,110,183,35,166,178,169,177,144,161,141,38,13,182,68,39,246,123,170,135,10,50,240,84,193,41,235,212,205,229,150,42,88,18,243,92,136,253,179,226,165,65,58,174,46,112,177,22,140,38,224,215,145,0,141,145,140,214,20,200,188,9,206,242,15,21,98,145,103,83,244,78,179,101,105,175,165,87,134,158,201,62,215,36,127,191,39,185,77,150,224,153,160,145,105,49,158,31,206,47,224,204,36,143,225,19,225,146,42,238,162,199,43,95,37,137,133,173,157,41,90,133,31,187,228,30,217,240,225,85,199,161,124,240,33,182,187,252,67,202,88,140,255,101,39,201,51,15,175,175,65,128,145,30,163,117,233,210,236,73,200,140,112,59,242,11,79,209,18,226,135,179,52,176,166,122,55,23,137,46]";

const ASSET: &str = "rgb1g2antx89ypjuat7jdth35d8xgqserckrhj9elkrhxhjhxch8sxqqguzmh6"; // STC, StableCoin
const GENESIS: &str = "genesis1qyfe883hey6jrgj2xvk5g3dfmfqfzm7a4wez4pd2krf7ltsxffd6u6nrvjvvnc8vt9llmp7663pgututl9heuwaudet72ay9j6thc6cetuvhxvsqqya5xjt2w9y4u6sfkuszwwctnrpug5yjxnthmr3mydg05rdrpspcxysnqvvqpfvag2w8jxzzsz9pf8pjfwf0xvln5z7w93yjln3gcnyxsa04jsf2p8vu4sxgeqrzyxg5nyvcysuur9qjct5xuzfvffyu23n6p22vaqpvcryvvqrnqvnswv7r3xqgxf3qryxransvgre33asjzqerx0vpe9lff02guztd6xyd5hwq0w37e0cqmutvm428mnmaayhlhfj4nh0zaalutdrurrnlets8axpxkfcqgpmrxqqqxsu7qc";

/// Test asset import
#[wasm_bindgen_test]
async fn asset_import() {
    set_panic_hook();

    // Import wallet
    resolve(save_mnemonic_seed(
        MNEMONIC.to_owned(),
        ENCRYPTION_PASSWORD.to_owned(),
        SEED_PASSWORD.to_owned(),
    ))
    .await;

    // Get vault properties
    let vault_str: JsValue = resolve(get_vault(
        ENCRYPTION_PASSWORD.to_owned(),
        ENCRYPTED_DESCRIPTORS.to_owned(),
    ))
    .await;
    let vault_data: VaultData = json_parse(&vault_str);

    resolve(import_asset(
        vault_data.descriptor.clone(),
        vault_data.change_descriptor.clone(),
        Some(ASSET.to_owned()),
        Some(GENESIS.to_owned()),
    ))
    .await;

    // Get wallet data
    let wallet_str: JsValue = resolve(get_wallet_data(
        vault_data.descriptor,
        vault_data.change_descriptor,
    ))
    .await;

    // Parse wallet data
    let wallet_data: WalletData = json_parse(&wallet_str);

    assert_eq!(wallet_data.transactions, vec![]);
}
