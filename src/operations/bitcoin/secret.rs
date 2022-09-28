use std::str::FromStr;

use anyhow::{anyhow, Result};
use bdk::{
    bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc},
    miniscript::{ScriptContext, Tap},
};
use bip39::Mnemonic;
use bitcoin::{secp256k1::Secp256k1, util::bip32::ChildNumber};

use crate::data::{
    constants::{BTC_PATH, NETWORK, RGB_ASSETS_PATH, RGB_UDAS_PATH},
    structs::VaultData,
};

fn get_random_buf() -> Result<[u8; 16], getrandom::Error> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}

fn get_descriptor<C: ScriptContext>(
    xprv: ExtendedPrivKey,
    path: &str,
    is_change: bool,
    is_secret: bool,
) -> Result<String> {
    let secp = Secp256k1::new();
    let deriv_descriptor: DerivationPath = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);
    let derived_xprv_desc_key: DescriptorKey<C> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(if is_change {
            1
        } else {
            0
        })?),
    )?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        if is_secret {
            Ok(desc_seckey.to_string())
        } else {
            let desc_pubkey = desc_seckey.as_public(&secp)?;
            Ok(desc_pubkey.to_string())
        }
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

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);
    let xpubkh = xpub.to_pub().pubkey_hash().to_string();

    let btc_descriptor_xprv = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, BTC_PATH, false, true)?
    );
    let btc_change_descriptor_xprv =
        format!("tr({})", get_descriptor::<Tap>(xprv, BTC_PATH, true, true)?);

    let btc_descriptor_xpub = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, BTC_PATH, false, false)?
    );
    let btc_change_descriptor_xpub = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, BTC_PATH, true, false)?
    );
    let rgb_assets_descriptor_xprv = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, RGB_ASSETS_PATH, false, true)?
    );
    let rgb_udas_descriptor_xprv = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, RGB_UDAS_PATH, false, true)?
    );
    let rgb_assets_descriptor_xpub = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, RGB_ASSETS_PATH, false, false)?
    );
    let rgb_udas_descriptor_xpub = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, RGB_UDAS_PATH, false, false)?
    );

    Ok(VaultData {
        btc_descriptor_xprv,
        btc_descriptor_xpub,
        btc_change_descriptor_xprv,
        btc_change_descriptor_xpub,
        rgb_assets_descriptor_xprv,
        rgb_assets_descriptor_xpub,
        rgb_udas_descriptor_xprv,
        rgb_udas_descriptor_xpub,
        xpubkh,
        mnemonic: mnemonic_phrase.to_string(),
    })
}
