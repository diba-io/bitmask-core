use anyhow::{anyhow, Context, Result};
use carbonado::{constants::Format, fs::Header, structs::Encoded};

pub async fn store(sk: &str, pk: &str, input: &[u8]) -> Result<()> {
    let carbonado_level = 15;
    let pubkey = hex::decode(pk)?;
    let sk = hex::decode(sk)?;

    log::info!("input bytes: {}", input.len());
    let Encoded(mut encoded, hash, encode_info) =
        carbonado::encode(&pubkey, input, carbonado_level)?;
    log::info!("encoded bytes: {}", encoded.len());

    let format = Format::try_from(carbonado_level)?;

    let header = Header::new(
        &sk,
        hash.as_bytes(),
        format,
        0,
        encode_info.output_len,
        encode_info.padding_len,
    )?;

    let mut body = header.try_to_vec()?;
    body.append(&mut encoded);

    let url = "/carbonado";
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .body(body)
        .header("Content-Type", "application/octet-stream")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    if status_code != 200 {
        Err(anyhow!(
            "Error in storing carbonado file, status: {status_code} error: {response_text}"
        ))
    } else {
        Ok(())
    }
}
