use anyhow::Result;
pub fn bech32_encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    use bech32::{encode, ToBase32, Variant};
    Ok(encode(hrp, bytes.to_base32(), Variant::Bech32m)?)
}

#[allow(dead_code)]
pub fn bech32_decode(bech32_str: &str) -> Result<(String, Vec<u8>)> {
    use bech32::{decode, FromBase32};
    let (hrp, words, _variant) = decode(bech32_str)?;
    Ok((hrp, Vec::<u8>::from_base32(&words)?))
}
