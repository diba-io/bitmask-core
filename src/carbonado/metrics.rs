#![cfg(not(target_arch = "wasm32"))]
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use log::{debug, error};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sled::{Config, Db, Mode};
use tokio::fs;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Default)]
pub struct MetricsData {
    bytes: u64,
    bytes_by_day: BTreeMap<String, u64>,
    bitcoin_wallets_by_day: BTreeMap<String, usize>,
    signet_wallets_by_day: BTreeMap<String, usize>,
    testnet_wallets_by_day: BTreeMap<String, usize>,
    regtest_wallets_by_day: BTreeMap<String, usize>,
    wallets_by_network: BTreeMap<String, usize>,
}

const MAINNET_WALLET: &str =
    "bitcoin-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const TESTNET_WALLET: &str =
    "testnet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const SIGNET_WALLET: &str =
    "signet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const REGTEST_WALLET: &str =
    "regtest-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const RGB_STOCK: &str =
    "bitcoin-4b1bc93ea7f03c49c4424b56561c9c7437e5f16e5714cece615a48e249264a84.c15";
const RGB_TRANSFER_FILE: &str =
    "bitcoin-28cc77c6e8e65def696101d839d0f335effa5da22d4a9c6d843bc6caa87e957e.c15";

const NETWORK_BITCOIN: &str = "bitcoin";
const NETWORK_TESTNET: &str = "testnet";
const NETWORK_SIGNET: &str = "signet";
const NETWORK_REGTEST: &str = "regtest";
const NETWORK_TOTAL: &str = "total";
const NETWORK_RGB_STOCKS: &str = "rgb_stocks";
const NETWORK_RGB_TRANSFER_FILES: &str = "rgb_transfer_files";

const DB_PATHS: &str = "PATHS";
const DB_DAYS: &str = "DAYS";

static METRICS_DB: Lazy<Db> = Lazy::new(|| {
    Config::default()
        .path(
            PathBuf::from(
                env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned()),
            )
            .join("metrics_sled_kv"),
        )
        .mode(Mode::HighThroughput) // Since this uses Tor, disk IO will not be a bottleneck
        .compression_factor(19)
        .open()
        .unwrap_or_else(|e| {
            error!(
                "Trouble opening Sled keystore: {}. Using a temporary in-memory database.",
                e
            );
            Config::default().temporary(true).open().unwrap()
        })
});

pub async fn init(dir: &Path) -> Result<()> {
    let mut metrics = MetricsData::default();

    metrics
        .wallets_by_network
        .insert(NETWORK_BITCOIN.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_TESTNET.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_SIGNET.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_REGTEST.to_string(), 0);
    metrics.wallets_by_network.insert("total".to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_RGB_STOCKS.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_RGB_TRANSFER_FILES.to_string(), 0);

    let mut total_wallets = 0;
    let mut rgb_stocks = 0;
    let mut rgb_transfer_files = 0;

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata()?;
        let day_created = metadata.created()?;
        let day = round_datetime_to_day(day_created.into());

        METRICS_DB
            .open_tree(DB_PATHS)?
            .insert(entry.path().to_str().unwrap_or("ERROR").as_bytes(), &[1])?;

        if metadata.is_file() {
            metrics.bytes += metadata.len();

            *metrics.bytes_by_day.entry(day.clone()).or_insert(0) += metadata.len();

            match filename.as_str() {
                MAINNET_WALLET => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_BITCOIN)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .bitcoin_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                TESTNET_WALLET => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_TESTNET)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .testnet_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                SIGNET_WALLET => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_SIGNET)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .signet_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                REGTEST_WALLET => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_REGTEST)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .regtest_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                RGB_STOCK => {
                    rgb_stocks += 1;
                }

                RGB_TRANSFER_FILE => {
                    rgb_transfer_files += 1;
                }

                _ => {}
            }
        }
    }

    *metrics
        .wallets_by_network
        .get_mut(NETWORK_TOTAL)
        .unwrap_or(&mut 0) = total_wallets;

    *metrics
        .wallets_by_network
        .get_mut(NETWORK_RGB_STOCKS)
        .unwrap_or(&mut 0) = rgb_stocks;

    *metrics
        .wallets_by_network
        .get_mut(NETWORK_RGB_TRANSFER_FILES)
        .unwrap_or(&mut 0) = rgb_transfer_files;

    let start_day = DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::from_ymd_opt(2023, 7, 25)
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

        let bytes_day_prior = {
            metrics
                .bytes_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .bytes_by_day
            .entry(day.clone())
            .and_modify(|b| *b += bytes_day_prior)
            .or_insert(bytes_day_prior);

        let bitcoin_wallets_day_prior = {
            metrics
                .bitcoin_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .bitcoin_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += bitcoin_wallets_day_prior)
            .or_insert(bitcoin_wallets_day_prior);

        let testnet_wallets_day_prior = {
            metrics
                .testnet_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .testnet_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += testnet_wallets_day_prior)
            .or_insert(testnet_wallets_day_prior);

        let signet_wallets_day_prior = {
            metrics
                .signet_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .signet_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += signet_wallets_day_prior)
            .or_insert(signet_wallets_day_prior);

        let regtest_wallets_day_prior = {
            metrics
                .regtest_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
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

    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());

    fs::write(&format!("{dir}/metrics.json"), &json(&metrics).await?).await?;
    fs::write(&format!("{dir}/metrics.csv"), &csv(&metrics).await).await?;

    Ok(())
}

pub async fn update(path: &Path) -> Result<()> {
    debug!("Updating metrics with {path:?}");

    if METRICS_DB
        .open_tree(DB_PATHS)?
        .contains_key(path.to_str().unwrap_or("ERROR").as_bytes())?
    {
        debug!("Path already present");
        return Ok(());
    } else {
        METRICS_DB
            .open_tree(DB_PATHS)?
            .insert(path.to_str().unwrap_or("ERROR").as_bytes(), &[1])?;
    }

    let filename = path
        .file_name()
        .ok_or(anyhow!("no filename for path"))?
        .to_string_lossy()
        .to_string();
    let metadata = path.metadata()?;
    let day_created = metadata.created()?;
    let day_prior = day_created
        .checked_sub(Duration::days(1).to_std()?)
        .expect("day exists");
    let day_prior = round_datetime_to_day(day_prior.into());
    let day = round_datetime_to_day(day_created.into());

    let first_of_day = if METRICS_DB
        .open_tree(DB_DAYS)?
        .contains_key(day.as_bytes())?
    {
        debug!("Day already present");
        false
    } else {
        METRICS_DB
            .open_tree(DB_DAYS)?
            .insert(day.as_bytes(), &[1])?;
        true
    };

    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let mut metrics: MetricsData =
        serde_json::from_str(&fs::read_to_string(format!("{dir}/metrics.json")).await?)?;

    if metadata.is_file() {
        if first_of_day {
            let bytes_day_prior = {
                metrics
                    .bytes_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .bytes_by_day
                .entry(day.clone())
                .and_modify(|b| *b += bytes_day_prior)
                .or_insert(bytes_day_prior);

            let bitcoin_wallets_day_prior = {
                metrics
                    .bitcoin_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .bitcoin_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += bitcoin_wallets_day_prior)
                .or_insert(bitcoin_wallets_day_prior);

            let testnet_wallets_day_prior = {
                metrics
                    .testnet_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .testnet_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += testnet_wallets_day_prior)
                .or_insert(testnet_wallets_day_prior);

            let signet_wallets_day_prior = {
                metrics
                    .signet_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .signet_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += signet_wallets_day_prior)
                .or_insert(signet_wallets_day_prior);

            let regtest_wallets_day_prior = {
                metrics
                    .regtest_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .regtest_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += regtest_wallets_day_prior)
                .or_insert(regtest_wallets_day_prior);
        }

        metrics.bytes += metadata.len();

        *metrics.bytes_by_day.entry(day.clone()).or_insert(0) += metadata.len();

        match filename.as_str() {
            MAINNET_WALLET => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_BITCOIN)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .bitcoin_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }
            TESTNET_WALLET => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TESTNET)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .testnet_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }
            SIGNET_WALLET => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_SIGNET)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .signet_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }
            REGTEST_WALLET => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_REGTEST)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .regtest_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }

            RGB_STOCK => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_RGB_STOCKS)
                    .unwrap_or(&mut 0) += 1;
            }

            RGB_TRANSFER_FILE => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_RGB_TRANSFER_FILES)
                    .unwrap_or(&mut 0) += 1;
            }

            _ => {}
        }
    }

    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());

    // Write metrics to disk as a backup
    fs::write(&format!("{dir}/metrics.json"), &json(&metrics).await?).await?;
    fs::write(&format!("{dir}/metrics.csv"), &csv(&metrics).await).await?;

    Ok(())
}

pub async fn csv(metrics: &MetricsData) -> String {
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

    for (day, bitcoin_wallets) in metrics.bitcoin_wallets_by_day.iter() {
        let mut line = vec![];

        if lines.len() == 1 {
            line.push(NETWORK_BITCOIN.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_BITCOIN)
                    .expect("network is defined")
                    .to_string(),
            );
            line.push(metrics.bytes.to_string());
        }

        if lines.len() == 2 {
            line.push(NETWORK_TESTNET.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_TESTNET)
                    .expect("network is defined")
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 3 {
            line.push(NETWORK_SIGNET.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_SIGNET)
                    .expect("network is defined")
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 4 {
            line.push(NETWORK_REGTEST.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_REGTEST)
                    .expect("network is defined")
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 5 {
            line.push(NETWORK_TOTAL.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_TOTAL)
                    .expect("total is defined")
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 6 {
            line.push(NETWORK_RGB_STOCKS.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_RGB_STOCKS)
                    .expect("rgb_stocks is defined")
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 7 {
            line.push(NETWORK_RGB_TRANSFER_FILES.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_RGB_TRANSFER_FILES)
                    .expect("rgb_transfer_files is defined")
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() > 7 {
            line.push("".to_owned());
            line.push("".to_owned());
            line.push("".to_owned());
        }

        line.push(day.clone());
        line.push(bitcoin_wallets.to_string());
        line.push(
            metrics
                .testnet_wallets_by_day
                .get(day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(
            metrics
                .signet_wallets_by_day
                .get(day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(
            metrics
                .regtest_wallets_by_day
                .get(day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(metrics.bytes_by_day.get(day).unwrap_or(&0).to_string());

        lines.push(line);
    }

    let lines: Vec<String> = lines.iter().map(|line| line.join(",")).collect();
    lines.join("\n")
}

pub async fn json(metrics: &MetricsData) -> Result<String> {
    Ok(serde_json::to_string_pretty(metrics)?)
}

fn round_datetime_to_day(datetime: DateTime<Utc>) -> String {
    let rounded = datetime
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid time");
    rounded.format("%Y-%m-%d").to_string()
}
