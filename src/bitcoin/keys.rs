use std::str::FromStr;

use anyhow::{anyhow, Result};
use bdk::{
    bitcoin::{
        secp256k1::Secp256k1,
        util::bip32::{ChildNumber, DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    },
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc, DescriptorSecretKey},
    miniscript::Tap,
};
use bip39::{Language, Mnemonic};
use bitcoin_hashes::{sha256, Hash};
use miniscript_crate::DescriptorPublicKey;
use nostr_sdk::prelude::{FromSkStr, ToBech32};

use crate::{
    constants::{BTC_PATH, NETWORK, NOSTR_PATH},
    structs::{EncryptedWalletData, PrivateWalletData, PublicWalletData},
};

fn get_descriptor(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<DescriptorSecretKey> {
    let secp = Secp256k1::new();
    let deriv_descriptor: DerivationPath = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);
    let derived_xprv_desc_key: DescriptorKey<Tap> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        Ok(desc_seckey)
    } else {
        Err(anyhow!("Unexpected key variant in get_descriptor"))
    }
}

fn xprv_desc(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<String> {
    let xprv = get_descriptor(xprv, path, change)?;

    Ok(format!("tr({xprv})"))
}

fn xpub_desc(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<String> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;

    Ok(format!("tr({xpub})"))
}

fn xpub_plain(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<String> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;

    if let DescriptorPublicKey::XPub(desc) = xpub {
        Ok(desc.xkey.to_string())
    } else {
        Err(anyhow!("Unexpected xpub descriptor"))
    }
}

fn nostr_keypair(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<(String, String)> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;

    if let DescriptorSecretKey::XPrv(desc_xkey) = xprv {
        let first_keypair = desc_xkey
            .xkey
            .ckd_priv(&secp, ChildNumber::from_normal_idx(0)?)?
            .to_keypair(&secp);

        Ok((
            hex::encode(first_keypair.secret_bytes()),
            hex::encode(first_keypair.x_only_public_key().0.serialize()),
        ))
    } else {
        Err(anyhow!("Unexpected key variant in nostr_keypair"))
    }
}

pub async fn new_mnemonic(seed_password: &str) -> Result<EncryptedWalletData> {
    let mut rng = bip39::rand::thread_rng();
    let mnemonic_phrase = Mnemonic::generate_in_with(&mut rng, Language::English, 12)?;

    get_mnemonic(mnemonic_phrase, seed_password).await
}

pub async fn save_mnemonic(
    mnemonic_phrase: &str,
    seed_password: &str,
) -> Result<EncryptedWalletData> {
    let mnemonic = Mnemonic::from_str(mnemonic_phrase)?;

    get_mnemonic(mnemonic, seed_password).await
}

pub async fn get_mnemonic(
    mnemonic_phrase: Mnemonic,
    seed_password: &str,
) -> Result<EncryptedWalletData> {
    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().await;
    let xprv = ExtendedPrivKey::new_master(*network, &seed)?;
    let xprvkh = sha256::Hash::hash(&xprv.to_priv().to_bytes()).to_string();

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);
    let xpubkh = xpub.to_pub().pubkey_hash().to_string();

    let btc_path = BTC_PATH.read().await;

    let btc_descriptor_xprv = xprv_desc(&xprv, &btc_path, 0)?;
    let btc_change_descriptor_xprv = xprv_desc(&xprv, &btc_path, 1)?;

    let btc_descriptor_xpub = xpub_desc(&xprv, &btc_path, 0)?;
    let btc_change_descriptor_xpub = xpub_desc(&xprv, &btc_path, 1)?;
    let rgb_assets_descriptor_xprv = xprv_desc(&xprv, &btc_path, 20)?;
    let rgb_udas_descriptor_xprv = xprv_desc(&xprv, &btc_path, 30)?;
    let rgb_assets_descriptor_xpub = xpub_desc(&xprv, &btc_path, 20)?;
    let rgb_udas_descriptor_xpub = xpub_desc(&xprv, &btc_path, 30)?;
    let xpub_plain = xpub_plain(&xprv, &btc_path, 0)?;

    let (nostr_prv, nostr_pub) = nostr_keypair(&xprv, NOSTR_PATH, 0)?;
    let nostr_keys = nostr_sdk::Keys::from_sk_str(&nostr_prv)?;
    let nostr_nsec = nostr_keys.secret_key()?.to_bech32()?;
    let nostr_npub = nostr_keys.public_key().to_bech32()?;

    let private = PrivateWalletData {
        btc_descriptor_xprv,
        btc_change_descriptor_xprv,
        rgb_assets_descriptor_xprv,
        rgb_udas_descriptor_xprv,
        nostr_prv,
        nostr_nsec,
    };

    let public = PublicWalletData {
        btc_descriptor_xpub,
        btc_change_descriptor_xpub,
        rgb_assets_descriptor_xpub,
        rgb_udas_descriptor_xpub,
        nostr_pub,
        nostr_npub,
        xprvkh,
        xpubkh,
        xpub: xpub_plain,
    };

    Ok(EncryptedWalletData {
        mnemonic: mnemonic_phrase.to_string(),
        private,
        public,
    })
}
