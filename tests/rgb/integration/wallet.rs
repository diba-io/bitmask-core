#![cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn allow_create_wallet() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_list_addresses() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_list_utxos() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_get_next_address() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_get_next_utxo() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_sync_wallet() -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
async fn allow_save_commitment() -> anyhow::Result<()> {
    Ok(())
}
