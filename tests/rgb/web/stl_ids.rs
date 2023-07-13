#![cfg(target_arch = "wasm32")]
use rgbstd::{
    interface::{rgb20_stl, rgb21_stl, rgb25_stl, LIB_ID_RGB20, LIB_ID_RGB21, LIB_ID_RGB25},
    stl::{rgb_contract_stl, rgb_std_stl, LIB_ID_RGB_CONTRACT, LIB_ID_RGB_STD},
};

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn check_rgb20_stl() {
    let schema = rgb20_stl();
    assert_eq!(LIB_ID_RGB20, schema.id().to_string());
}

#[wasm_bindgen_test]
async fn check_rgb21_stl() {
    let schema = rgb21_stl();
    assert_eq!(LIB_ID_RGB21, schema.id().to_string());
}

#[wasm_bindgen_test]
async fn check_rgb25_stl() {
    let schema = rgb25_stl();
    assert_eq!(LIB_ID_RGB25, schema.id().to_string());
}

#[wasm_bindgen_test]
async fn check_rgbstd_stl() {
    let schema = rgb_std_stl();
    assert_eq!(LIB_ID_RGB_STD, schema.id().to_string());
}

#[wasm_bindgen_test]
async fn check_rgbcontract_stl() {
    let schema = rgb_contract_stl();
    assert_eq!(LIB_ID_RGB_CONTRACT, schema.id().to_string());
}
