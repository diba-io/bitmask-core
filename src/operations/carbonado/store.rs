use anyhow::{anyhow, Context, Result};

pub async fn store(sk: &str, pk: &str, input: &[u8]) -> Result<()> {
    let level = 15;
    let pk = hex::decode(pk)?;
    let sk = hex::decode(sk)?;

    let (body, _encode_info) = carbonado::file::encode(&sk, Some(&pk), input, level)?;

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

    if status_code != 200 {
        let response_text = response.text().await.context(format!(
            "Error in parsing server response for POST JSON request to {url}"
        ))?;

        Err(anyhow!(
            "Error in storing carbonado file, status: {status_code} error: {response_text}"
        ))
    } else {
        Ok(())
    }
}
