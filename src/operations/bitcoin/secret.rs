use std::str::FromStr;

use anyhow::{anyhow, Result};
use bdk::{
    bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc},
    miniscript::Tap,
};
use bip39::Mnemonic;
use bitcoin::{secp256k1::Secp256k1, util::bip32::ChildNumber};
use bitcoin_hashes::{sha256, Hash};

use crate::data::{
    constants::{BTC_PATH, NETWORK, NOSTR_PATH},
    structs::{EncryptedWalletData, PrivateWalletData, PublicWalletData},
};

fn get_random_buf() -> Result<[u8; 16], getrandom::Error> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}

fn get_descriptor(
    xprv: &ExtendedPrivKey,
    path: &str,
    change: u32,
    is_secret: bool,
) -> Result<String> {
    let secp = Secp256k1::new();
    let deriv_descriptor: DerivationPath = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);
    let derived_xprv_desc_key: DescriptorKey<Tap> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        if is_secret {
            Ok(desc_seckey.to_string())
        } else {
            let desc_pubkey = desc_seckey.to_public(&secp)?;
            Ok(desc_pubkey.to_string())
        }
    } else {
        Err(anyhow!("Invalid key variant"))
    }
}

pub fn new_mnemonic(seed_password: &str) -> Result<EncryptedWalletData> {
    let entropy = get_random_buf()?;
    let mnemonic_phrase = Mnemonic::from_entropy(&entropy)?;
    get_mnemonic(mnemonic_phrase, seed_password)
}

pub fn save_mnemonic(mnemonic_phrase: &str, seed_password: &str) -> Result<EncryptedWalletData> {
    let mnemonic = Mnemonic::from_str(mnemonic_phrase)?;
    get_mnemonic(mnemonic, seed_password)
}

pub fn get_mnemonic(mnemonic_phrase: Mnemonic, seed_password: &str) -> Result<EncryptedWalletData> {
    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().unwrap();
    let xprv = ExtendedPrivKey::new_master(*network, &seed)?;
    let xprvkh = sha256::Hash::hash(&xprv.to_priv().to_bytes()).to_string();

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);
    let xpubkh = xpub.to_pub().pubkey_hash().to_string();

    let btc_path = BTC_PATH.read().unwrap();

    let btc_descriptor_xprv = format!("tr({})", get_descriptor(&xprv, &btc_path, 0, true)?);
    let btc_change_descriptor_xprv = format!("tr({})", get_descriptor(&xprv, &btc_path, 1, true)?);

    let btc_descriptor_xpub = format!("tr({})", get_descriptor(&xprv, &btc_path, 0, false)?);
    let btc_change_descriptor_xpub = format!("tr({})", get_descriptor(&xprv, &btc_path, 1, false)?);
    let rgb_assets_descriptor_xprv = format!("tr({})", get_descriptor(&xprv, &btc_path, 20, true)?);
    let rgb_udas_descriptor_xprv = format!("tr({})", get_descriptor(&xprv, &btc_path, 30, true)?);
    let rgb_assets_descriptor_xpub =
        format!("tr({})", get_descriptor(&xprv, &btc_path, 20, false)?);
    let rgb_udas_descriptor_xpub = format!("tr({})", get_descriptor(&xprv, &btc_path, 30, false)?);

    let nostr_prv = get_descriptor(&xprv, NOSTR_PATH, 0, true)?;
    let nostr_pub = get_descriptor(&xprv, NOSTR_PATH, 0, false)?;

    // TODO: nostr hex keys and npub/nsec.

    let public = PublicWalletData {
        btc_descriptor_xpub,
        btc_change_descriptor_xpub,
        rgb_assets_descriptor_xpub,
        rgb_udas_descriptor_xpub,
        nostr_pub,
        xprvkh,
        xpubkh,
    };

    let private = PrivateWalletData {
        btc_descriptor_xprv,
        btc_change_descriptor_xprv,
        rgb_assets_descriptor_xprv,
        rgb_udas_descriptor_xprv,
        nostr_prv,
        mnemonic: mnemonic_phrase.to_string(),
    };

    Ok(EncryptedWalletData { private, public })
}
