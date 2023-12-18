#![cfg(not(target_arch = "wasm32"))]
use std::{
    env, fs, path,
    process::{Command, Stdio},
};

use anyhow::Result;

pub fn init_fs() -> Result<()> {
    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let dir = path::Path::new(&dir);
    fs::create_dir_all(dir)?;

    Ok(())
}

pub fn send_coins(address: &str, amount: &str) {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/regtest/send_coins.sh", path);
    Command::new("bash")
        .arg(full_file)
        .args([address, amount])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .expect("oh no!");
}

pub fn new_block() {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/regtest/new_block.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .expect("oh no!");
}
