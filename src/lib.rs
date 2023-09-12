#[macro_use]
extern crate amplify;

mod bitcoin;

// Explicit exports to keep secrets private
pub use crate::bitcoin::{
    create_payjoin, create_transaction, decrypt_wallet, drain_wallet, dust_tx, encrypt_wallet,
    fund_vault, get_assets_vault, get_blockchain, get_new_address, get_wallet, get_wallet_data,
    hash_password, new_mnemonic, new_wallet, save_mnemonic, send_sats, sign_psbt, sign_psbt_file,
    sign_psbt_with_multiple_wallets, sync_wallet, sync_wallets, upgrade_wallet,
    versioned_descriptor, BitcoinError,
};

pub mod carbonado;
pub mod constants;
pub mod error;
pub mod lightning;
pub mod nostr;
#[cfg(not(target_arch = "wasm32"))]
pub mod regtest;
pub mod rgb;
pub mod structs;
pub mod util;
pub mod validators;
#[cfg(target_arch = "wasm32")]
pub mod web;
