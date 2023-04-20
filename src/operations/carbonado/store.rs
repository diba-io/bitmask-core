use anyhow::Result;
use carbonado::structs::Encoded;

pub fn store(bytes: &[u8], pk: &[u8]) -> Result<()> {
    log::info!("input bytes: {}", bytes.len());
    let Encoded(encoded, _, _) = carbonado::encode(pk, bytes, 15)?;
    log::info!("input bytes: {}", encoded.len());

    Ok(())
}
