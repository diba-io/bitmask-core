# BitMask Core

Core functionality for the BitMask wallet - <https://bitmask.app>

## Uses

- [bdk](https://github.com/bitcoindevkit/bdk) - Bitcoin Dev Kit
- [gloo](https://github.com/rustwasm/gloo)
- [wasm-pack](https://github.com/rustwasm/wasm-pack)

## Build

This should work with either wasm-pack, trunk, or x86-64.

Some environment variables may be needed in order to compile on macos-aarch64, for more, [see this](https://github.com/sapio-lang/sapio/issues/146#issuecomment-960659800).

If there are issues compiling, be sure to check you're compiling with the latest Rust version.

## Test

1. Lint against wasm32: `cargo clippy --target wasm32-unknown-unknown`
2. Run tests in browser: `wasm-pack test --headless --chrome`
