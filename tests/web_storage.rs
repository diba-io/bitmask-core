#![cfg(all(target_arch = "wasm32"))]
use wasm_bindgen_test::*;

use bitmask_core::{
    info,
    web::{
        carbonado::{retrieve, store},
        json_parse, resolve, set_panic_hook,
    },
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn web_storage() {
    set_panic_hook();

    let sk = "76e9a09d5fa501c9048cb7ff48415786f7f6580726f33823010d130b19f61680".to_owned();
    let name = "test my@rly$neat_file".to_owned();
    let data = b"Hello world!".to_vec();

    info!("Testing web data store");
    resolve(store(sk.clone(), name.clone(), data.clone())).await;

    info!("Testing web data retrieve");
    let result = resolve(retrieve(sk, name)).await;

    info!("Parsing result");
    let result: Vec<u8> = json_parse(&result);

    assert_eq!(data, result, "Data stored and data retrieved match");
}
