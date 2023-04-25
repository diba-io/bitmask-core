use anyhow::{anyhow, Context, Result};

pub async fn retrieve(sk: &str, pk: &str) -> Result<Vec<u8>> {
    let secret_key = hex::decode(sk)?;

    let url = format!("/carbonado/{pk}.c15");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    if status_code != 200 {
        let response_text = response.text().await.context(format!(
            "Error in parsing server response for POST JSON request to {url}"
        ))?;
        return Err(anyhow!(
            "Error in storing carbonado file, status: {status_code} error: {response_text}"
        ));
    }

    let encoded = response.bytes().await?;

    let (_header, decoded) = carbonado::file::decode(&secret_key, &encoded)?;

    Ok(decoded)
}
