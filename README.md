# BitMask Core
Core functionality for the BitMask wallet - <https://bitmask.app>

**BitMask** is a bitcoin wallet and a browser extension for accessing decentralized web applications on the Bitcoin blokchain. It is designed to support UTXO based smart contracting protocols such as RGB, with planned support for Omni layer, TARO and many others.

[![Build status](https://img.shields.io/github/actions/workflow/status/diba-io/bitmask-core/rust.yaml?branch=development&style=flat-square)](https://github.com/diba-io/bitmask-core/actions/workflows/rust.yaml)
[![Crates.io](https://img.shields.io/crates/v/bitmask-core?style=flat-square)](https://docs.rs/bitmask-core/latest/bitmask-core/)
[![npm: bitmask-core](https://img.shields.io/npm/v/bitmask-core?style=flat-square)](https://www.npmjs.com/package/bitmask-core)
[![License: MIT+APACHE](https://img.shields.io/crates/l/bitmask-core?style=flat-square)](https://mit-license.org)
[![Telegram](https://img.shields.io/badge/telegram-invite-blue?style=flat-square)](https://t.me/+eQk5aQ5--iUxYzVk)

## Uses

- [bdk](https://github.com/bitcoindevkit/bdk) - Bitcoin Dev Kit
- [gloo](https://github.com/rustwasm/gloo)
- [wasm-pack](https://github.com/rustwasm/wasm-pack)

## Build

This should work with either wasm-pack, trunk, or x86-64.

Some environment variables may be needed in order to compile on macos-aarch64, for more, [see this](https://github.com/sapio-lang/sapio/issues/146#issuecomment-960659800).

If there are issues compiling, be sure to check you're compiling with the latest Rust version.

To build this as a NodeJS module, use: `wasm-pack build --release --target bundler`

## Test

1. Lint against wasm32: `cargo clippy --target wasm32-unknown-unknown`
2. Run tests in browser: `TEST_WALLET_SEED="replace with a 12 word mnemonic for a wallet containing testnet sats" wasm-pack test --headless --chrome`

## Run

To run the bitmaskd node with REST server, either for testing the web wallet, or simply for increased privacy:

`cargo install --features=server --path .`

Then run `bitmaskd`.

## Development

Parts of this application are built with conditional compilation statements for wasm32 support. This is a helpful command for checking linting and correctness while also developing on desktop platforms:

`cargo clippy --target wasm32-unknown-unknown --no-default-features --release`

## Release

Upon a new release, follow these steps:

1. Run `cargo update` to update to latest deps.
1. Run `cargo +nightly udeps` to see if there are any unused dependencies.

## Docker

For running bitmask-core tests in regtest, please follow the steps bellow:

1. Build bitcoin node + electrum: `docker compose build`
2. Up and running containers: `docker compose up -d node1`
3. Load the command line: `source .commands`
4. Send some coins to main wallet address: `node1 sendtoaddress {ADDRESS} 10`
5. Mine a block: `node1 -generate`
6. Running the tests: `TEST_WALLET_SEED="replace with a 12 word mnemonic for a wallet containing testnet sats" cargo test allow_transfer -- --test-threads 1`

### Troubleshooting

#### **1. After restarting the container**

**A.The bitcoin node does not work?**

Check if your wallet is loaded. For that, run the command `node1 loadwallet default`.

**B.The electrs node does not work?**

To stop the electrs freeze, run `node1 -generate`.
