use std::{env, process::Stdio};

use tokio::process::Command;

pub const REGTEST_MNEMONIC: &str =
    "ordinary crucial edit settle pencil lion appear unlock left fly century license";

pub async fn start_node() {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/startup_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

pub async fn stop_node() {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/stop_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}

pub async fn send_some_coins(address: &str, amount: &str) {
    let path = env::current_dir().expect("");
    let path = path.to_str().expect("");
    let full_file = format!("{}/tests/scripts/send_coins.sh", path);
    Command::new("bash")
        .arg(full_file)
        .args(&[address, amount])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("");
}
