#![cfg(target_arch = "wasm32")]
use bp::bc::stl::bitcoin_stl;
use rgb::interface::{rgb20_stl, rgb21_stl};
use rgbstd::stl::rgb_contract_stl;
use strict_types::stl::std_stl;

use wasm_bindgen_test::*;

use bitmask_core::web::set_panic_hook;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn allow_load_stl() {
    set_panic_hook();
    std_stl();
}

#[wasm_bindgen_test]
async fn allow_load_bitcoin() {
    set_panic_hook();
    bitcoin_stl();
}

#[wasm_bindgen_test]
async fn allow_load_contracts_stl() {
    set_panic_hook();
    rgb_contract_stl();
}

#[wasm_bindgen_test]
async fn allow_load_rgb20_stl() {
    set_panic_hook();
    rgb20_stl();
}

#[wasm_bindgen_test]
async fn allow_load_rgb21_stl() {
    set_panic_hook();
    rgb21_stl();
}
