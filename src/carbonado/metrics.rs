use std::{collections::BTreeMap, path::Path, time::SystemTime};

use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Default)]
pub struct MetricsResponse {
    bytes: u64,
    bytes_by_day: BTreeMap<String, u64>,
    bitcoin_wallets_by_day: BTreeMap<String, usize>,
    signet_wallets_by_day: BTreeMap<String, usize>,
    testnet_wallets_by_day: BTreeMap<String, usize>,
    regtest_wallets_by_day: BTreeMap<String, usize>,
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

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata()?;
        let day_created = metadata.created()?;
        let day = round_datetime_to_day(day_created.into());

        if metadata.is_file() {
            response.bytes += metadata.len();

            *response.bytes_by_day.entry(day.clone()).or_insert(0) += metadata.len();

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
                    .or_insert(0) += 1;
            }

            if filename
                == "testnet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("testnet")
                    .unwrap_or(&mut 0) += 1;
                *response
                    .testnet_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
            }

            if filename
                == "signet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("signet")
                    .unwrap_or(&mut 0) += 1;
                *response
                    .signet_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
            }

            if filename
                == "regtest-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                *response
                    .wallets_by_network
                    .get_mut("regtest")
                    .unwrap_or(&mut 0) += 1;
                *response
                    .regtest_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
            }

            if filename == "bitcoin-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
                || filename == "testnet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
                || filename == "signet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
                || filename == "regtest-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15"
            {
                total_wallets += 1;
            }
        }
    }

    *response
        .wallets_by_network
        .get_mut("total")
        .unwrap_or(&mut 0) = total_wallets;

    let start_day = DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::from_ymd_opt(2023, 7, 1)
            .expect("correct date")
            .and_hms_opt(0, 0, 0)
            .expect("correct time"),
        Utc,
    );
    let end_day: DateTime<Utc> = SystemTime::now().into();
    let end_day = round_datetime_to_day(end_day);

    let mut d = 0;

    loop {
        let start_day = start_day + Duration::days(d);
        let day_prior = start_day - Duration::days(1);
        let day_prior = round_datetime_to_day(day_prior);
        let day = round_datetime_to_day(start_day);

        let mut bytes_day_prior = {
            response
                .bytes_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        *response
            .bytes_by_day
            .get_mut(&day)
            .unwrap_or(&mut bytes_day_prior) = bytes_day_prior;

        let bitcoin_wallets_day_prior = {
            response
                .bitcoin_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        response
            .bitcoin_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += bitcoin_wallets_day_prior)
            .or_insert(bitcoin_wallets_day_prior);

        let testnet_wallets_day_prior = {
            response
                .testnet_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        response
            .testnet_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += testnet_wallets_day_prior)
            .or_insert(testnet_wallets_day_prior);

        let signet_wallets_day_prior = {
            response
                .signet_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        response
            .signet_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += signet_wallets_day_prior)
            .or_insert(signet_wallets_day_prior);

        let regtest_wallets_day_prior = {
            response
                .regtest_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        response
            .regtest_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += regtest_wallets_day_prior)
            .or_insert(regtest_wallets_day_prior);

        if day == end_day {
            break;
        } else {
            d += 1;
        }
    }

    Ok(response)
}

pub fn metrics_csv(metrics: MetricsResponse) -> String {
    let mut lines = vec![vec![
        "Wallet".to_owned(),
        "Wallet Count".to_owned(),
        "Bytes Total".to_owned(),
        "Day".to_owned(),
        "Bitcoin Wallets by Day".to_owned(),
        "Testnet Wallets by Day".to_owned(),
        "Signet Wallets by Day".to_owned(),
        "Regtest Wallets by Day".to_owned(),
        "Bytes by Day".to_owned(),
    ]];

    for (day, bitcoin_wallets) in metrics.bitcoin_wallets_by_day {
        let mut line = vec![];

        if lines.len() == 1 {
            line.push("Bitcoin".to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get("bitcoin")
                    .expect("network is defined")
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
                    .expect("network is defined")
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
                    .expect("network is defined")
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
                    .expect("network is defined")
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
        line.push(
            metrics
                .testnet_wallets_by_day
                .get(&day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(
            metrics
                .signet_wallets_by_day
                .get(&day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(
            metrics
                .regtest_wallets_by_day
                .get(&day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(metrics.bytes_by_day.get(&day).unwrap_or(&0).to_string());

        lines.push(line);
    }

    let lines: Vec<String> = lines.iter().map(|line| line.join(",")).collect();
    lines.join("\n")
}

fn round_datetime_to_day(datetime: DateTime<Utc>) -> String {
    let rounded = datetime
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid time");
    rounded.format("%Y-%m-%d").to_string()
}
