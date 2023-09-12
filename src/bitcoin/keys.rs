use std::{pin::Pin, str::FromStr};

use bdk::{
    bitcoin::{
        secp256k1::Secp256k1,
        util::bip32::{ChildNumber, DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    },
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc, DescriptorSecretKey},
    miniscript::{descriptor::DescriptorKeyParseError, Tap},
};
use bip39::{Language, Mnemonic};
use bitcoin::KeyPair;
use bitcoin_hashes::{sha256, Hash};
use miniscript_crate::DescriptorPublicKey;
use nostr_sdk::prelude::{FromSkStr, ToBech32};
use once_cell::sync::OnceCell;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    constants::{BTC_PATH, NETWORK},
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
    let derived_xprv_desc_key: DescriptorKey<Tap> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        Ok(desc_seckey)
    } else {
        Err(BitcoinKeysError::UnexpectedKey)
    }
}

fn xprv_desc(
    xprv: &ExtendedPrivKey,
    path: &str,
    change: u32,
) -> Result<SecretString, BitcoinKeysError> {
    let xprv = get_descriptor(xprv, path, change)?;

    Ok(SecretString(format!("tr({xprv})")))
}

fn xpub_desc(
    xprv: &ExtendedPrivKey,
    path: &str,
    change: u32,
) -> Result<SecretString, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;

    Ok(SecretString(format!("tr({xpub})")))
}

fn watcher_xpub(
    xprv: &ExtendedPrivKey,
    path: &str,
    change: u32,
) -> Result<SecretString, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;

    if let DescriptorPublicKey::XPub(desc) = xpub {
        Ok(SecretString(desc.xkey.to_string()))
    } else {
        Err(BitcoinKeysError::UnexpectedWatcherXpubDescriptor)
    }
}

pub static NOSTR_SK: OnceCell<Pin<Zeroizing<[u8; 32]>>> = OnceCell::new();

// For NIP-06 Nostr signing and Carbonado encryption key derivation
fn nostr_keypair(xprv: &ExtendedPrivKey) -> Result<(SecretString, SecretString), BitcoinKeysError> {
    pub const NOSTR_PATH: &str = "m/44'/1237'/0'/0/0";
    let deriv_descriptor = DerivationPath::from_str(NOSTR_PATH)?;
    let secp = Secp256k1::new();
    let nostr_sk = xprv.derive_priv(&secp, &deriv_descriptor)?;
    let keypair =
        KeyPair::from_seckey_slice(&secp, nostr_sk.private_key.secret_bytes().as_slice())?;

    let mut sk = nostr_sk.private_key.secret_bytes();
    let _ = NOSTR_SK.set(Pin::new(Zeroizing::new(sk)));

    let sk_str = SecretString(hex::encode(sk));
    sk.zeroize();

    let mut pk = keypair.x_only_public_key().0.serialize();
    let pk_str = SecretString(hex::encode(pk));
    pk.zeroize();

    Ok((sk_str, pk_str))
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
    let mut xprvkh_bytes = xprv.to_priv().to_bytes();
    let xprvkh = SecretString(sha256::Hash::hash(&xprvkh_bytes).to_string());
    xprvkh_bytes.zeroize();

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);
    let xpubkh = SecretString(xpub.to_pub().pubkey_hash().to_string());

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
    let nostr_keys = nostr_sdk::Keys::from_sk_str(&nostr_prv.0)?;
    let nostr_nsec = SecretString(nostr_keys.secret_key()?.to_bech32()?);
    let nostr_npub = SecretString(nostr_keys.public_key().to_bech32()?);

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
        xpub: SecretString(xpub.to_string()),
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
