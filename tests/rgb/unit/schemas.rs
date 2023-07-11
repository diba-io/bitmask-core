#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::util::init_logging;
use rgb_schemata::nia_schema;

#[tokio::test]
async fn check_nia_schema_id() -> Result<()> {
    init_logging("rgb_issue=warn");
    let expected = "5YWfKW3CqANHsKqpxy3HaCpt5bgvsMXHUuXiHpoynEYG";
    let shema = nia_schema();
    assert_eq!(expected, shema.schema_id().to_string());
    Ok(())
}
