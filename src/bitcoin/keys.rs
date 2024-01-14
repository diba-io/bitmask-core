use std::str::FromStr;

#[cfg(feature = "segwit")]
use bdk::miniscript::Segwitv0;
#[cfg(not(feature = "segwit"))]
use bdk::miniscript::Tap;
use bdk::{
    bitcoin::{
        secp256k1::Secp256k1,
        util::bip32::{ChildNumber, DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    },
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc, DescriptorSecretKey},
    miniscript::descriptor::DescriptorKeyParseError,
};
use bip39::{Language, Mnemonic};
use bitcoin::{KeyPair, Network};
use bitcoin_hashes::{sha256, Hash};
use miniscript_crate::{
    descriptor::{DescriptorXKey, Wildcard},
    DescriptorPublicKey,
};
use nostr_sdk::prelude::{FromSkStr, ToBech32};
use thiserror::Error;
use zeroize::Zeroize;

use crate::{
    constants::{get_marketplace_fee_xpub, get_network, BTC_PATH, NETWORK},
    structs::{DecryptedWalletData, PrivateWalletData, PublicWalletData, SecretString},
};

#[derive(Error, Debug)]
pub enum BitcoinKeysError {
    /// Unexpected key variant in get_descriptor
    #[error("Unexpected key variant in get_descriptor")]
    UnexpectedKey,
    /// Unexpected xpub descriptor
    #[error("Unexpected xpub descriptor")]
    UnexpectedWatcherXpubDescriptor,
    /// Unexpected key variant in nostr_keypair
    #[error("Unexpected key variant in nostr_keypair")]
    UnexpectedKeyVariantInNostrKeypair,
    /// secp256k1 error
    #[error(transparent)]
    Secp256k1Error(#[from] bitcoin::secp256k1::Error),
    /// BIP-32 error
    #[error(transparent)]
    Bip32Error(#[from] bitcoin::util::bip32::Error),
    /// BIP-39 error
    #[error(transparent)]
    Bip39Error(#[from] bip39::Error),
    /// BDK key error
    #[error(transparent)]
    BdkKeyError(#[from] bdk::keys::KeyError),
    /// Miniscript descriptor key parse error
    #[error(transparent)]
    MiniscriptDescriptorKeyParseError(#[from] DescriptorKeyParseError),
    /// getrandom error
    #[error(transparent)]
    GetRandomError(#[from] getrandom::Error),
    /// Nostr SDK key error
    #[error(transparent)]
    NostrKeyError(#[from] nostr_sdk::key::Error),
    /// Nostr SDK key error
    #[error(transparent)]
    NostrNip19Error(#[from] nostr_sdk::nips::nip19::Error),
}

fn get_descriptor(
    xprv: &ExtendedPrivKey,
    path: &str,
    change: u32,
) -> Result<DescriptorSecretKey, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let deriv_descriptor = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);
    #[cfg(not(feature = "segwit"))]
    let derived_xprv_desc_key: DescriptorKey<Tap> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;
    #[cfg(feature = "segwit")]
    let derived_xprv_desc_key: DescriptorKey<Segwitv0> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        Ok(desc_seckey)
    } else {
        Err(BitcoinKeysError::UnexpectedKey)
    }
}

fn xprv_desc(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
    let xprv = get_descriptor(xprv, path, change)?;

    #[cfg(not(feature = "segwit"))]
    Ok(format!("tr({xprv})"))
    #[cfg(feature = "segwit")]
    Ok(format!("wpkh({xprv})"))
}

fn xpub_desc(xprv: &ExtendedPrivKey, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;

    #[cfg(not(feature = "segwit"))]
    Ok(format!("tr({xprv})"))
    #[cfg(feature = "segwit")]
    Ok(format!("wpkh({xprv})"))
}

fn watcher_xpub(
    xprv: &ExtendedPrivKey,
    path: &str,
    change: u32,
) -> Result<String, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;

    if let DescriptorPublicKey::XPub(desc) = xpub {
        Ok(desc.xkey.to_string())
    } else {
        Err(BitcoinKeysError::UnexpectedWatcherXpubDescriptor)
    }
}

// For NIP-06 Nostr signing and Carbonado encryption key derivation
fn nostr_keypair(xprv: &ExtendedPrivKey) -> Result<(String, String), BitcoinKeysError> {
    pub const NOSTR_PATH: &str = "m/44'/1237'/0'/0/0";
    let deriv_descriptor = DerivationPath::from_str(NOSTR_PATH)?;
    let secp = Secp256k1::new();
    let nostr_sk = xprv.derive_priv(&secp, &deriv_descriptor)?;
    let keypair =
        KeyPair::from_seckey_slice(&secp, nostr_sk.private_key.secret_bytes().as_slice())?;

    Ok((
        hex::encode(nostr_sk.private_key.secret_bytes()),
        hex::encode(keypair.x_only_public_key().0.serialize()),
    ))
}

pub async fn new_mnemonic(
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinKeysError> {
    let mut entropy = [0u8; 32];
    getrandom::getrandom(&mut entropy)?;
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)?;
    entropy.zeroize();

    get_mnemonic(mnemonic, seed_password).await
}

pub async fn save_mnemonic(
    mnemonic_phrase: &SecretString,
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinKeysError> {
    let mnemonic = Mnemonic::from_str(&mnemonic_phrase.0)?;

    get_mnemonic(mnemonic, seed_password).await
}

pub async fn get_mnemonic(
    mnemonic_phrase: Mnemonic,
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinKeysError> {
    let seed = mnemonic_phrase.to_seed_normalized(&seed_password.0);

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
    let rgb_udas_descriptor_xprv = xprv_desc(&xprv, &btc_path, 21)?;
    let rgb_assets_descriptor_xpub = xpub_desc(&xprv, &btc_path, 20)?;
    let rgb_udas_descriptor_xpub = xpub_desc(&xprv, &btc_path, 21)?;
    let watcher_xpub = watcher_xpub(&xprv, &btc_path, 0)?;

    let (nostr_prv, nostr_pub) = nostr_keypair(&xprv)?;
    let nostr_keys = nostr_sdk::Keys::from_sk_str(&nostr_prv)?;
    let nostr_nsec = nostr_keys.secret_key()?.to_bech32()?;
    let nostr_npub = nostr_keys.public_key().to_bech32()?;

    let private = PrivateWalletData {
        xprvkh,
        btc_descriptor_xprv,
        btc_change_descriptor_xprv,
        rgb_assets_descriptor_xprv,
        rgb_udas_descriptor_xprv,
        nostr_prv,
        nostr_nsec,
    };

    let public = PublicWalletData {
        xpub: xpub.to_string(),
        xpubkh,
        watcher_xpub,
        btc_descriptor_xpub,
        btc_change_descriptor_xpub,
        rgb_assets_descriptor_xpub,
        rgb_udas_descriptor_xpub,
        nostr_pub,
        nostr_npub,
    };

    Ok(DecryptedWalletData {
        mnemonic: mnemonic_phrase.to_string(),
        private,
        public,
    })
}

pub async fn get_marketplace_descriptor() -> Result<Option<SecretString>, BitcoinKeysError> {
    let btc_path = BTC_PATH.read().await;
    let marketplace_xpub = get_marketplace_fee_xpub().await;
    let network = get_network().await;
    let network = Network::from_str(&network).expect("wrong network");

    if marketplace_xpub.is_empty() {
        return Ok(None);
    }

    let path = DerivationPath::from_str(&btc_path)?;
    let deriv = DerivationPath::default().child(ChildNumber::from_normal_idx(0)?);
    let mut xkey =
        ExtendedPubKey::from_str(&marketplace_xpub).expect("wrong marketplace xpub format");
    xkey.network = network;

    let fp = xkey.fingerprint();
    let origin: KeySource = (fp, path.clone());

    let desc = DescriptorXKey::<ExtendedPubKey> {
        origin: Some(origin),
        xkey,
        derivation_path: deriv,
        wildcard: Wildcard::None,
    };

    let desc_xpub = DescriptorPublicKey::XPub(desc).to_string();

    #[cfg(not(feature = "segwit"))]
    let desc_str = format!("tr({desc_xpub}/*)");
    #[cfg(feature = "segwit")]
    let desc_str = format!("wpkh({desc_xpub}/*)");

    Ok(Some(SecretString(desc_str)))
}
