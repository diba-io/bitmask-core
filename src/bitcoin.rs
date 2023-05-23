use std::str::FromStr;

use ::bitcoin::util::address::Address;
use ::psbt::Psbt;
use anyhow::{anyhow, Result};
use argon2::Argon2;
use bdk::{wallet::AddressIndex, FeeRate, LocalUtxo, TransactionDetails};
use bitcoin::psbt::PartiallySignedTransaction;
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use tokio::try_join;

mod assets;
mod keys;
mod payment;
mod psbt;
mod wallet;

pub use crate::bitcoin::{
    assets::dust_tx,
    keys::{new_mnemonic, save_mnemonic},
    payment::{create_payjoin, create_transaction},
    psbt::sign_psbt,
    wallet::{get_blockchain, get_wallet, synchronize_wallet},
};

use crate::{
    constants::{DIBA_DESCRIPTOR, DIBA_DESCRIPTOR_VERSION, DIBA_MAGIC_NO},
    debug, info,
    structs::{
        EncryptedWalletData, EncryptedWalletDataV04, FundVaultDetails, MnemonicSeedData,
        SatsInvoice, SignPsbtRequest, SignPsbtResponse, WalletData, WalletTransaction,
    },
    trace,
};

impl SerdeEncryptSharedKey for EncryptedWalletData {
    type S = BincodeSerializer<Self>;
}

impl SerdeEncryptSharedKey for EncryptedWalletDataV04 {
    type S = BincodeSerializer<Self>;
}

/// Bitcoin Wallet Operations

const BITMASK_ARGON2_SALT: &[u8] = b"DIBA BitMask Password Hash"; // Never change this

pub fn hash_password(password: &str) -> String {
    use argon2::{Algorithm, Params, Version};

    let mut output_key_material = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
        .hash_password_into(
            password.as_bytes(),
            BITMASK_ARGON2_SALT,
            &mut output_key_material,
        )
        .expect("Password hashed with Argon2id");

    hex::encode(output_key_material)
}

pub fn get_encrypted_wallet(
    hash: &str,
    encrypted_descriptors: &str,
) -> Result<EncryptedWalletData> {
    let shared_key: [u8; 32] = hex::decode(hash)?
        .try_into()
        .expect("hash is of fixed size");
    let encrypted_descriptors: Vec<u8> = hex::decode(encrypted_descriptors)?;
    let (version_prefix, encrypted_descriptors) = encrypted_descriptors.split_at(5);

    if !version_prefix.starts_with(&DIBA_MAGIC_NO) {
        return Err(anyhow!(
            "Wrong Format: Encrypted descriptor is not prefixed with DIBA magic number. Prefix was: {version_prefix:?}"
        ));
    }

    if version_prefix[4] != DIBA_DESCRIPTOR_VERSION {
        return Err(anyhow!(
            "Wrong Version: Encrypted descriptor is the wrong version. The version byte was: {}",
            version_prefix[4]
        ));
    }

    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors.to_owned())?;

    Ok(EncryptedWalletData::decrypt_owned(
        &encrypted_message,
        &SharedKey::from_array(shared_key),
    )?)
}

pub async fn upgrade_wallet(
    hash: &str,
    encrypted_descriptors: &str,
    seed_password: &str,
) -> Result<String> {
    // read hash digest and consume hasher
    let shared_key: [u8; 32] = hex::decode(hash)?
        .try_into()
        .expect("hash is of fixed size");
    let encrypted_descriptors: Vec<u8> = hex::decode(encrypted_descriptors)?;
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors)?;

    let descriptor = match EncryptedWalletData::decrypt_owned(
        &encrypted_message,
        &SharedKey::from_array(shared_key),
    ) {
        Ok(_data) => None,
        Err(_err) => {
            // If there's a deserialization error, attempt to recover just the mnemnonic.
            let recovered_wallet_data = EncryptedWalletDataV04::decrypt_owned(
                &encrypted_message,
                &SharedKey::from_array(shared_key),
            )?;

            // println!("Recovered wallet data: {recovered_wallet_data:?}"); // Keep commented out for security

            let upgraded_descriptor =
                save_mnemonic_seed(&recovered_wallet_data.mnemonic, hash, seed_password).await?;

            Some(upgraded_descriptor.encrypted_descriptors)
        }
    };

    // if descriptor.is_none() {
    //     todo!("Add later version migrations here");
    // }

    match descriptor {
        Some(result) => Ok(result),
        None => Err(anyhow!("Descriptor does not need to be upgraded")),
    }
}

pub fn versioned_descriptor(encrypted_message: EncryptedMessage) -> String {
    let mut descriptor_data = DIBA_DESCRIPTOR.to_vec();
    let mut encrypted_descriptors = encrypted_message.serialize();
    descriptor_data.append(&mut encrypted_descriptors);

    hex::encode(descriptor_data)
}

pub async fn new_mnemonic_seed(hash: &str, seed_password: &str) -> Result<MnemonicSeedData> {
    let shared_key: [u8; 32] = hex::decode(hash)?
        .try_into()
        .expect("hash is of fixed size");
    let wallet_data = new_mnemonic(seed_password).await?;
    let encrypted_message = wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let encrypted_descriptors = versioned_descriptor(encrypted_message);

    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: wallet_data.mnemonic,
        encrypted_descriptors,
    };

    Ok(mnemonic_seed_data)
}

pub async fn save_mnemonic_seed(
    mnemonic_phrase: &str,
    hash: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let shared_key: [u8; 32] = hex::decode(hash)?
        .try_into()
        .expect("hash is of fixed size");
    let wallet_data = save_mnemonic(mnemonic_phrase, seed_password).await?;
    let encrypted_message = wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let encrypted_descriptors = versioned_descriptor(encrypted_message);

    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: wallet_data.mnemonic,
        encrypted_descriptors,
    };

    Ok(mnemonic_seed_data)
}

pub async fn get_wallet_data(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<WalletData> {
    info!("get_wallet_data");
    info!(format!("descriptor: {descriptor}"));
    info!(format!("change_descriptor {change_descriptor:?}"));

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    synchronize_wallet(&wallet).await?;
    let address = wallet.get_address(AddressIndex::LastUnused)?.to_string();
    info!(format!("address: {address}"));
    let balance = wallet.get_balance()?;
    info!(format!("balance: {balance:?}"));
    let utxos = wallet.list_unspent().unwrap_or_default();
    let utxos: Vec<String> = utxos.into_iter().map(|x| x.outpoint.to_string()).collect();
    trace!(format!("unspent: {utxos:#?}"));

    let mut transactions = wallet.list_transactions(false).unwrap_or_default();
    trace!(format!("transactions: {transactions:#?}"));

    transactions.sort_by(|a, b| {
        b.confirmation_time
            .as_ref()
            .map(|t| t.height)
            .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
    });

    let transactions: Vec<WalletTransaction> = transactions
        .into_iter()
        .map(|tx| WalletTransaction {
            txid: tx.txid,
            received: tx.received,
            sent: tx.sent,
            fee: tx.fee,
            confirmed: tx.confirmation_time.is_some(),
            confirmation_time: tx.confirmation_time,
        })
        .collect();

    Ok(WalletData {
        address,
        balance,
        transactions,
        utxos,
    })
}

pub async fn get_new_address(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<String> {
    info!("get_new_address");
    info!(format!("descriptor: {descriptor}"));
    info!(format!("change_descriptor: {change_descriptor:?}"));

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    synchronize_wallet(&wallet).await?;
    let address = wallet.get_address(AddressIndex::New)?.to_string();
    info!(format!("address: {address}"));
    Ok(address)
}

pub async fn send_sats(
    descriptor: &str,
    change_descriptor: &str,
    destination: &str, // bip21 uri or address
    amount: u64,
    fee_rate: Option<f32>,
) -> Result<TransactionDetails> {
    use payjoin::UriExt;

    let wallet = get_wallet(descriptor, Some(change_descriptor.to_owned())).await?;
    synchronize_wallet(&wallet).await?;

    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let transaction = match payjoin::Uri::try_from(destination) {
        Ok(uri) => {
            let address = uri.address.clone();
            if let Ok(pj_uri) = uri.check_pj_supported() {
                create_payjoin(
                    vec![SatsInvoice { address, amount }],
                    &wallet,
                    fee_rate,
                    pj_uri,
                )
                .await?
            } else {
                create_transaction(vec![SatsInvoice { address, amount }], &wallet, fee_rate).await?
            }
        }
        _ => {
            let address = Address::from_str(destination)?;
            create_transaction(vec![SatsInvoice { address, amount }], &wallet, fee_rate).await?
        }
    };

    Ok(transaction)
}

pub async fn fund_vault(
    btc_descriptor_xprv: &str,
    btc_change_descriptor_xprv: &str,
    assets_address: &str,
    uda_address: &str,
    asset_amount: u64,
    uda_amount: u64,
    fee_rate: Option<f32>,
) -> Result<FundVaultDetails> {
    let assets_address = Address::from_str(assets_address)?;
    let uda_address = Address::from_str(uda_address)?;

    let wallet = get_wallet(
        btc_descriptor_xprv,
        Some(btc_change_descriptor_xprv.to_owned()),
    )
    .await?;
    synchronize_wallet(&wallet).await?;

    let asset_invoice = SatsInvoice {
        address: assets_address,
        amount: asset_amount,
    };
    let uda_invoice = SatsInvoice {
        address: uda_address,
        amount: uda_amount,
    };

    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let asset_tx_details = create_transaction(
        vec![asset_invoice.clone(), asset_invoice],
        &wallet,
        fee_rate,
    )
    .await?;

    let uda_tx_details =
        create_transaction(vec![uda_invoice.clone(), uda_invoice], &wallet, fee_rate).await?;

    let asset_txid = asset_tx_details.txid;
    let asset_outputs: Vec<String> = asset_tx_details
        .transaction
        .expect("asset tx exists")
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{asset_txid}:{i}"))
        .collect();

    let uda_txid = uda_tx_details.txid;
    let uda_outputs: Vec<String> = uda_tx_details
        .transaction
        .expect("uda tx exists")
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{uda_txid}:{i}"))
        .collect();

    Ok(FundVaultDetails {
        assets_output: Some(asset_outputs[0].to_owned()),
        assets_change_output: Some(asset_outputs[1].to_owned()),
        udas_output: Some(uda_outputs[0].to_owned()),
        udas_change_output: Some(uda_outputs[1].to_owned()),
        is_funded: true,
    })
}

fn utxo_string(utxo: &LocalUtxo) -> String {
    utxo.outpoint.to_string()
}

pub async fn get_assets_vault(
    rgb_assets_descriptor_xpub: &str,
    rgb_udas_descriptor_xpub: &str,
) -> Result<FundVaultDetails> {
    let assets_wallet = get_wallet(rgb_assets_descriptor_xpub, None).await?;
    let udas_wallet = get_wallet(rgb_udas_descriptor_xpub, None).await?;

    try_join!(
        synchronize_wallet(&assets_wallet),
        synchronize_wallet(&udas_wallet)
    )?;

    let assets_utxos = assets_wallet.list_unspent()?;
    let uda_utxos = udas_wallet.list_unspent()?;

    debug!(format!("Asset UTXOs: {assets_utxos:#?}"));
    debug!(format!("UDA UTXOs: {uda_utxos:#?}"));

    let mut assets_utxos: Vec<String> = assets_utxos.iter().map(utxo_string).collect();
    assets_utxos.sort();

    let mut uda_utxos: Vec<String> = uda_utxos.iter().map(utxo_string).collect();
    uda_utxos.sort();

    let assets_change_output = assets_utxos.pop();
    let assets_output = assets_utxos.pop();
    let udas_change_output = uda_utxos.pop();
    let udas_output = uda_utxos.pop();

    let is_funded = assets_change_output.is_some()
        && assets_output.is_some()
        && udas_change_output.is_some()
        && udas_output.is_some();

    Ok(FundVaultDetails {
        assets_output,
        assets_change_output,
        udas_output,
        udas_change_output,
        is_funded,
    })
}

pub async fn sign_psbt_file(_sk: &str, request: SignPsbtRequest) -> Result<SignPsbtResponse> {
    let SignPsbtRequest {
        psbt,
        mnemonic,
        seed_password,
        iface,
    } = request;

    let original_psbt = Psbt::from_str(&psbt)?;
    let final_psbt = PartiallySignedTransaction::from(original_psbt);

    // TODO: Refactor this!
    let encrypt_wallet = save_mnemonic(&mnemonic, &seed_password).await?;
    let sk = match iface.as_str() {
        "RGB20" => encrypt_wallet.private.rgb_assets_descriptor_xprv,
        "RGB21" => encrypt_wallet.private.rgb_udas_descriptor_xprv,
        _ => encrypt_wallet.private.rgb_assets_descriptor_xprv,
    };

    let wallet = get_wallet(&sk, None).await?;
    synchronize_wallet(&wallet).await?;

    let sign = sign_psbt(&wallet, final_psbt).await?;
    let resp = match sign.transaction {
        Some(tx) => SignPsbtResponse {
            sign: true,
            txid: tx.txid().to_string(),
        },
        _ => SignPsbtResponse {
            sign: false,
            txid: String::new(),
        },
    };

    Ok(resp)
}
