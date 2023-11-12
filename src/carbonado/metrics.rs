use std::{collections::BTreeMap, path::Path, time::SystemTime};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Default)]
pub struct MetricsResponse {
    bytes: u64,
    bytes_by_day: BTreeMap<String, u64>,
    bitcoin_wallets_by_day: BTreeMap<String, usize>,
    wallets_by_network: BTreeMap<String, usize>,
}

pub fn metrics(dir: &Path) -> Result<MetricsResponse> {
    let mut response = MetricsResponse::default();

    response.wallets_by_network.insert("bitcoin".to_string(), 0);
    response.wallets_by_network.insert("testnet".to_string(), 0);
    response.wallets_by_network.insert("signet".to_string(), 0);
    response.wallets_by_network.insert("regtest".to_string(), 0);
    response.wallets_by_network.insert("total".to_string(), 0);

    let mut total_wallets = 0;
    let mut day_prior = None;

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata()?;
        let day = metadata.created()?;
        let day = round_system_time_to_day(day);

        if metadata.is_file() {
            response.bytes += metadata.len();

            let bytes_total_day_prior = if let Some(dp) = &day_prior {
                response.bytes_by_day.get(dp).unwrap_or(&0).to_owned()
            } else {
                0
            };

            let bitcoin_wallets_total_day_prior = if let Some(dp) = day_prior {
                response
                    .bitcoin_wallets_by_day
                    .get(&dp)
                    .unwrap_or(&0)
                    .to_owned()
            } else {
                0
            };

            *response
                .bytes_by_day
                .entry(day.clone())
                .or_insert(bytes_total_day_prior) += metadata.len();

            if filename
                == "bitcoin-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("bitcoin")
                    .unwrap_or(&mut 0) += 1;
                *response
                    .bitcoin_wallets_by_day
                    .entry(day.clone())
                    .or_insert(bitcoin_wallets_total_day_prior) += 1;
            }

            if filename
                == "testnet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("testnet")
                    .unwrap_or(&mut 0) += 1;
            }

            if filename
                == "signet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("signet")
                    .unwrap_or(&mut 0) += 1;
            }

            if filename
                == "regtest-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("regtest")
                    .unwrap_or(&mut 0) += 1;
            }

            if filename == "bitcoin-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
                || filename == "testnet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
                || filename == "signet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
                || filename == "regtest-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                total_wallets += 1;
            }
        }

        day_prior = Some(day);
    }

    *response
        .wallets_by_network
        .get_mut("total")
        .unwrap_or(&mut 0) = total_wallets;

    Ok(response)
}

pub fn metrics_csv(metrics: MetricsResponse) -> String {
    let lines = vec![vec![
        "Wallet",
        "Wallet Count",
        "Bytes Total",
        "Day",
        "Bitcoin Wallets by Day",
        "Bytes by Day",
    ]];

    for (day, bitcoin_wallets) in metrics.bitcoin_wallets_by_day {
        let mut line = vec![];

        if lines.len() == 1 {
            line.push("Bitcoin".to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get("bitcoin")
                    .unwrap()
                    .to_string(),
            );
            line.push(metrics.bytes.to_string());
        }

        if lines.len() == 2 {
            line.push("Tesnet".to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get("testnet")
                    .unwrap()
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 3 {
            line.push("signet".to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get("signet")
                    .unwrap()
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 4 {
            line.push("regtest".to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get("regtest")
                    .unwrap()
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() > 4 {
            line.push("".to_owned());
            line.push("".to_owned());
            line.push("".to_owned());
        }

        line.push(day.clone());
        line.push(bitcoin_wallets.to_string());
        line.push(metrics.bytes_by_day.get(&day).unwrap().to_string())
    }

    let lines: Vec<String> = lines.iter().map(|line| line.join(",")).collect();
    lines.join("\n")
}

fn round_system_time_to_day(system_time: SystemTime) -> String {
    // Convert SystemTime to a Chrono DateTime
    let datetime: DateTime<Utc> = system_time.into();

    // Round down to the nearest day (00:00:00)
    let rounded = datetime.date_naive().and_hms_opt(0, 0, 0).unwrap();

    // Format the date as a string in "YYYY-MM-DD" format
    rounded.format("%Y-%m-%d").to_string()
}
