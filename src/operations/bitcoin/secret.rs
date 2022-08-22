use std::str::FromStr;

use anyhow::Result;
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

use crate::data::constants::{BTC_CHANGE_PATH, BTC_PATH, NETWORK, RGB_NFTS_PATH, RGB_TOKENS_PATH};

fn get_random_buf() -> Result<[u8; 16], getrandom::Error> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}

fn get_descriptor<C: ScriptContext>(xprv: ExtendedPrivKey, path: &str, xpub: bool) -> String {
    let secp = Secp256k1::new();
    let deriv_descriptor: DerivationPath = DerivationPath::from_str(path).unwrap();
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor).unwrap();

    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);

    let derived_xprv_desc_key: DescriptorKey<C> = derived_xprv
        .into_descriptor_key(Some(origin), DerivationPath::default())
        .unwrap();

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        let desc_pubkey = desc_seckey.as_public(&secp).unwrap();
        if xpub {
            desc_pubkey.to_string()
        } else {
            desc_seckey.to_string()
        }
    } else {
        "Invalid key variant".to_string()
    }
}

// fn get_rgb_descriptor(network: &str, master_xpriv: ExtendedPrivKey, path: u32) -> String {
//     let secp = Secp256k1::new();
//     let master_xpub = ExtendedPubKey::from_priv(&secp, &master_xpriv);
//     let scheme = Bip43::Bip86;
//     let blockchain = DerivationBlockchain::from_str(network).unwrap();
//     let derivation =
//         scheme.to_account_derivation(ChildNumber::from_hardened_idx(path).unwrap(), blockchain);
//     let account_xpriv = master_xpriv.derive_priv(&secp, &derivation).unwrap();
//     let account =
//         MemorySigningAccount::with(&secp, master_xpub.identifier(), derivation, account_xpriv);
//     let descriptor = account.recommended_descriptor();
//     descriptor.unwrap().to_string()
// }

pub fn get_mnemonic(seed_password: &str) -> (String, String, String, String, String, String) {
    let entropy = get_random_buf().expect("Get browser entropy");
    let mnemonic_phrase =
        Mnemonic::from_entropy(&entropy).expect("New mnemonic from browser entropy");

    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().unwrap();
    let xprv = ExtendedPrivKey::new_master(*network, &seed).expect("New xprivkey from seed");

    let btc_descriptor = format!("tr({})", get_descriptor::<Tap>(xprv, BTC_PATH, false));
    let btc_change_descriptor = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, BTC_CHANGE_PATH, false)
    );
    let rgb_tokens_descriptor =
        format!("tr({})", get_descriptor::<Tap>(xprv, RGB_TOKENS_PATH, true));
    let rgb_nfts_descriptor = format!("tr({})", get_descriptor::<Tap>(xprv, RGB_NFTS_PATH, true));

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);

    (
        mnemonic_phrase.to_string(),
        btc_descriptor,
        btc_change_descriptor,
        rgb_tokens_descriptor,
        rgb_nfts_descriptor,
        xpub.to_pub().pubkey_hash().to_string(),
    )
}

pub fn save_mnemonic(
    seed_password: &str,
    mnemonic: &str,
) -> (String, String, String, String, String) {
    let mnemonic_phrase = Mnemonic::from_str(mnemonic).expect("Parse mnemonic seed phrase");

    let seed = mnemonic_phrase.to_seed_normalized(seed_password);

    let network = NETWORK.read().unwrap();
    let xprv = ExtendedPrivKey::new_master(*network, &seed).expect("New xprivkey from seed");

    let btc_descriptor = format!("tr({})", get_descriptor::<Tap>(xprv, BTC_PATH, false));
    let btc_change_descriptor = format!(
        "tr({})",
        get_descriptor::<Tap>(xprv, BTC_CHANGE_PATH, false)
    );
    let rgb_tokens_descriptor =
        format!("tr({})", get_descriptor::<Tap>(xprv, RGB_TOKENS_PATH, true));
    let rgb_nfts_descriptor = format!("tr({})", get_descriptor::<Tap>(xprv, RGB_NFTS_PATH, true));

    let secp = Secp256k1::new();
    let xpub = ExtendedPubKey::from_priv(&secp, &xprv);

    (
        btc_descriptor,
        btc_change_descriptor,
        rgb_tokens_descriptor,
        rgb_nfts_descriptor,
        xpub.to_pub().pubkey_hash().to_string(),
    )
}
