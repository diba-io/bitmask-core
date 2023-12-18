#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::regtest::init_fs;

#[tokio::test]
pub async fn init() -> Result<()> {
    init_fs().await?;

    Ok(())
}
