#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::util::init_logging;

use crate::rgb::integration::utils::{setup_integration, shutdown_integration};

#[tokio::test]
async fn allow_utxo_selection() -> anyhow::Result<()> {
    init_logging("rgb_issue=warn");
    let _rgb_wallet = setup_integration().await?;

    shutdown_integration().await?;
    Ok(())
}
