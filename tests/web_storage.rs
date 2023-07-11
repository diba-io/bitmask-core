#![cfg(all(target_arch = "wasm32"))]
use bitmask_core::{
    info,
    web::{
        carbonado::{retrieve, store},
        resolve, set_panic_hook,
    },
};
use js_sys::Uint8Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn web_storage() {
    set_panic_hook();

    let sk = "76e9a09d5fa501c9048cb7ff48415786f7f6580726f33823010d130b19f61680".to_owned();
    let name = "test-my-file.c15".to_owned();
    let data = b"Hello world!".to_vec();

    info!("Testing web data store");
    resolve(store(sk.clone(), name.clone(), data.clone(), false, None)).await;

    info!("Testing web data retrieve");
    let result: JsValue = resolve(retrieve(sk, name)).await;
    let array = Uint8Array::new(&result);
    let bytes: Vec<u8> = array.to_vec();
    assert_eq!(data, bytes, "Data stored and data retrieved match");
}
