use std::str::FromStr;

use anyhow::Result;
use bdk::{
    bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey, KeySource},
    descriptor::Segwitv0,
    keys::{DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc},
};
use bip39::Mnemonic;
use bitcoin::secp256k1::Secp256k1;

use crate::data::constants::{NETWORK, STRING_CHANGE_DESCRIPTOR, STRING_DESCRIPTOR};

fn get_random_buf() -> Result<[u8; 16], getrandom::Error> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}

fn get_descriptor(xprv: ExtendedPrivKey, path: String) -> String {
    let secp = Secp256k1::new();
    let deriv_path: DerivationPath = DerivationPath::from_str(path.as_str()).unwrap();
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_path).unwrap();

    let origin: KeySource = (xprv.fingerprint(&secp), deriv_path);

    let derived_xprv_desc_key: DescriptorKey<Segwitv0> = derived_xprv
        .into_descriptor_key(Some(origin), DerivationPath::default())
        .unwrap();

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        let _desc_pubkey = desc_seckey.as_public(&secp).unwrap();
        desc_seckey.to_string()
    } else {
        "Invalid key variant".to_string()
    }
}

pub fn get_mnemonic(seed_password: &str) -> (String, String, String, String) {
    let entropy = get_random_buf().expect("Get browser entropy");
    let mnemonic_phrase =
        Mnemonic::from_entropy(&entropy).expect("New mnemonic from browser entropy");

    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().unwrap();
    let xprv = ExtendedPrivKey::new_master(*network, &seed).expect("New xprivkey from seed");

    let descriptor = format!(
        "wpkh({})",
        get_descriptor(xprv, STRING_DESCRIPTOR.to_string())
    );
    let change_descriptor = format!(
        "wpkh({})",
        get_descriptor(xprv, STRING_CHANGE_DESCRIPTOR.to_string())
    );

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_private(&secp, &xprv);

    (
        mnemonic_phrase.to_string(),
        descriptor,
        change_descriptor,
        xpub.public_key.pubkey_hash().to_string(),
    )
}

pub fn save_mnemonic(seed_password: &str, mnemonic: String) -> (String, String, String) {
    let mnemonic_phrase = Mnemonic::from_str(&mnemonic).expect("Parse mnemonic seed phrase");

    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().unwrap();
    let xprv = ExtendedPrivKey::new_master(*network, &seed).expect("New xprivkey from seed");

    let descriptor = format!(
        "wpkh({})",
        get_descriptor(xprv, STRING_DESCRIPTOR.to_string())
    );
    let change_descriptor = format!(
        "wpkh({})",
        get_descriptor(xprv, STRING_CHANGE_DESCRIPTOR.to_string())
    );

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_private(&secp, &xprv);

    (
        descriptor,
        change_descriptor,
        xpub.public_key.pubkey_hash().to_string(),
    )
}
