use std::str::FromStr;

use anyhow::{anyhow, Result};
use bdk::{
    bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc},
    miniscript::{ScriptContext, Tap},
};
use bip39::Mnemonic;
use bitcoin::secp256k1::Secp256k1;
// use bitcoin::util::bip32::ChildNumber;
// use psbt::sign::MemorySigningAccount;
// use wallet::hd::{standards::DerivationBlockchain, Bip43, DerivationStandard};

use crate::data::{
    constants::{BTC_PATH, NETWORK, RGB_ASSETS_PATH, RGB_UDAS_PATH},
    structs::VaultData,
};

fn get_random_buf() -> Result<[u8; 16], getrandom::Error> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}

fn get_descriptor<C: ScriptContext>(xprv: ExtendedPrivKey, path: &str) -> Result<String> {
    let secp = Secp256k1::new();
    let deriv_descriptor: DerivationPath = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);
    let derived_xprv_desc_key: DescriptorKey<C> =
        derived_xprv.into_descriptor_key(Some(origin), DerivationPath::default())?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        let desc_pubkey = desc_seckey.as_public(&secp)?;
        Ok(desc_pubkey.to_string())
    } else {
        Err(anyhow!("Invalid key variant"))
    }
}

pub fn new_mnemonic(seed_password: &str) -> Result<VaultData> {
    let entropy = get_random_buf()?;
    let mnemonic_phrase = Mnemonic::from_entropy(&entropy)?;
    get_mnemonic(mnemonic_phrase, seed_password)
}

pub fn save_mnemonic(seed_password: &str, mnemonic: &str) -> Result<VaultData> {
    let mnemonic_phrase = Mnemonic::from_str(mnemonic).expect("Parse mnemonic seed phrase");
    get_mnemonic(mnemonic_phrase, seed_password)
}

pub fn get_mnemonic(mnemonic_phrase: Mnemonic, seed_password: &str) -> Result<VaultData> {
    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().unwrap();
    let xprv = ExtendedPrivKey::new_master(*network, &seed)?;

    let btc_descriptor = format!("tr({}/0/*)", get_descriptor::<Tap>(xprv, BTC_PATH)?);
    let btc_change_descriptor = format!("tr({}/1/*)", get_descriptor::<Tap>(xprv, BTC_PATH)?);
    let rgb_assets_descriptor =
        format!("tr({}/0/*)", get_descriptor::<Tap>(xprv, RGB_ASSETS_PATH)?);
    let rgb_assets_change_descriptor =
        format!("tr({}/1/*)", get_descriptor::<Tap>(xprv, RGB_ASSETS_PATH)?);
    let rgb_udas_descriptor = format!("tr({}/0/*)", get_descriptor::<Tap>(xprv, RGB_UDAS_PATH)?);
    let rgb_udas_change_descriptor =
        format!("tr({}/1/*)", get_descriptor::<Tap>(xprv, RGB_UDAS_PATH)?);

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);

    Ok(VaultData {
        btc_descriptor,
        btc_change_descriptor,
        rgb_assets_descriptor,
        rgb_assets_change_descriptor,
        rgb_udas_descriptor,
        rgb_udas_change_descriptor,
        xpubkh: xpub.to_pub().pubkey_hash().to_string(),
        mnemonic: mnemonic_phrase.to_string(),
    })
}
