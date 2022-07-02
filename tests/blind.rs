#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::set_blinded_utxo;
use log::info;
use std::env;

/// Test asset import
#[tokio::test]
async fn asset_import() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    pretty_env_logger::init();

    let blinded =
        set_blinded_utxo("0b199e9bbbb79a9a1bc8d9a59d0f02f9eef045c2923577e719739d2546f7296e:2")?;

    info!("blinded utxo: {blinded:?}");

    Ok(())
}
