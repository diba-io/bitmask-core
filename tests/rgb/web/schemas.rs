#![cfg(target_arch = "wasm32")]
use bitmask_core::web::set_panic_hook;
use wasm_bindgen_test::*;

use rgb_schemata::nia_schema;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn check_nia_schema_id() {
    set_panic_hook();
    let expected = "5YWfKW3CqANHsKqpxy3HaCpt5bgvsMXHUuXiHpoynEYG";
    let shema = nia_schema();
    assert_eq!(expected, shema.schema_id().to_string());
}
