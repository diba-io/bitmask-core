use std::str::FromStr;

use ::bitcoin::util::address::Address;
use ::psbt::Psbt;
use argon2::Argon2;
use bdk::{wallet::AddressIndex, FeeRate, LocalUtxo, SignOptions, TransactionDetails};
use bitcoin::psbt::PartiallySignedTransaction;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use thiserror::Error;
use zeroize::Zeroize;

mod assets;
mod keys;
mod payment;
mod psbt;
mod wallet;

pub use crate::bitcoin::{
    assets::dust_tx,
    keys::{new_mnemonic, save_mnemonic, BitcoinKeysError},
    payment::{create_payjoin, create_transaction, BitcoinPaymentError},
    psbt::{sign_psbt, sign_psbt_with_multiple_wallets, BitcoinPsbtError},
    wallet::{
        get_blockchain, get_wallet, sync_wallet, sync_wallets, BitcoinWalletError, MemoryWallet,
    },
};

use crate::{
    constants::{DIBA_DESCRIPTOR, DIBA_DESCRIPTOR_VERSION, DIBA_MAGIC_NO, NETWORK},
    debug, info,
    structs::{
        DecryptedWalletData, EncryptedWalletDataV04, FundVaultDetails, SatsInvoice, SecretString,
        SignPsbtRequest, SignPsbtResponse, WalletData, WalletTransaction,
    },
    trace,
};

// Minimum amount of satoshis to funding vault
const MIN_FUNDS_SATS: u64 = 10000;

impl SerdeEncryptSharedKey for DecryptedWalletData {
    type S = BincodeSerializer<Self>;
}

impl SerdeEncryptSharedKey for EncryptedWalletDataV04 {
    type S = BincodeSerializer<Self>;
}

#[derive(Error, Debug)]
pub enum BitcoinError {
    /// Wrong Encrypted Descriptor Format
    #[error(
        "Wrong Format: Encrypted descriptor is not prefixed with DIBA magic number. Prefix was: {0}"
    )]
    WrongEncryptedDescriptorMagicNo(String),
    /// Wrong Encrypted Descriptor Version
    #[error("Wrong Version: Encrypted descriptor is the wrong version. The version byte was: {0}")]
    WrongEncryptedDescriptorVersion(u8),
    #[error("Wrong Descriptor: Wallet supports only taproot descriptor")]
    WrongDescriptor,
    /// Upgrade unnecessary
    #[error("Descriptor does not need to be upgraded")]
    UpgradeUnnecessary,
    /// Wrong network
    #[error("Address provided is on the wrong network!")]
    WrongNetwork,
    /// Wrong Encrypted Descriptor Format
    #[error("Insufficient satoshis to create funding wallet. Minimum: {0} sats")]
    InsufficientFundSats(u64),
    /// Drain wallet unable to finalize PSBT
    #[error("Drain wallet was unable to finalize PSBT")]
    DrainWalletUnfinalizedPsbt,
    /// Drain wallet was unable to find tx details
    #[error("No wallet transaction details were found when draining wallet")]
    DrainWalletNoTxDetails,
    /// BitMask Core Bitcoin Keys error
    #[error(transparent)]
    BitcoinKeysError(#[from] BitcoinKeysError),
    /// BitMask Core Bitcoin Payment error
    #[error(transparent)]
    BitcoinPaymentError(#[from] BitcoinPaymentError),
    /// BitMask Core Bitcoin Psbt error
    #[error(transparent)]
    BitcoinPsbtError(#[from] BitcoinPsbtError),
    /// BitMask Core Bitcoin Wallet error
    #[error(transparent)]
    BitcoinWalletError(#[from] BitcoinWalletError),
    /// hex decode error
    #[error(transparent)]
    HexDecodeError(#[from] hex::FromHexError),
    /// serde encrypt error
    #[error(transparent)]
    SerdeEncryptError(#[from] serde_encrypt::Error),
    /// BDK error
    #[error(transparent)]
    BdkError(#[from] bdk::Error),
    /// BDK esplora error
    #[error(transparent)]
    BdkEsploraError(#[from] bdk::esplora_client::Error),
    /// Bitcoin address error
    #[error(transparent)]
    BitcoinAddressError(#[from] bitcoin::util::address::Error),
    /// PSBT decode error
    #[error(transparent)]
    BitcoinPsbtDecodeError(#[from] bitcoin::consensus::encode::Error),
}

/// Bitcoin Wallet Operations
const BITMASK_ARGON2_SALT: &[u8] = b"DIBA BitMask Password Hash"; // Never change this

pub fn hash_password(password: &SecretString) -> SecretString {
    use argon2::{Algorithm, Params, Version};

    let mut output_key_material = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
        .hash_password_into(
            password.0.as_bytes(),
            BITMASK_ARGON2_SALT,
            &mut output_key_material,
        )
        .expect("Password hashed with Argon2id");

    let hash = SecretString(hex::encode(output_key_material));
    output_key_material.zeroize();
    hash
}

pub fn decrypt_wallet(
    hash: &SecretString,
    encrypted_descriptors: &SecretString,
) -> Result<DecryptedWalletData, BitcoinError> {
    let mut shared_key: [u8; 32] = hex::decode(&hash.0)?
        .try_into()
        .expect("hash is of fixed size");
    let encrypted_descriptors: Vec<u8> = hex::decode(&encrypted_descriptors.0)?;
    let (version_prefix, encrypted_descriptors) = encrypted_descriptors.split_at(5);

    if !version_prefix.starts_with(&DIBA_MAGIC_NO) {
        return Err(BitcoinError::WrongEncryptedDescriptorMagicNo(format!(
            "{version_prefix:#?}"
        )));
    }

    if version_prefix[4] != DIBA_DESCRIPTOR_VERSION {
        return Err(BitcoinError::WrongEncryptedDescriptorVersion(
            version_prefix[4],
        ));
    }

    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors.to_owned())?;

    let decrypted_wallet_data =
        DecryptedWalletData::decrypt_owned(&encrypted_message, &SharedKey::from_array(shared_key))?;

    shared_key.zeroize();

    Ok(decrypted_wallet_data)
}

pub async fn upgrade_wallet(
    hash: &SecretString,
    encrypted_descriptors: &SecretString,
    seed_password: &SecretString,
) -> Result<SecretString, BitcoinError> {
    // read hash digest and consume hasher
    let shared_key: [u8; 32] = hex::decode(&hash.0)?
        .try_into()
        .expect("hash is of fixed size");
    let encrypted_descriptors: Vec<u8> = hex::decode(&encrypted_descriptors.0)?;
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors)?;

    match DecryptedWalletData::decrypt_owned(&encrypted_message, &SharedKey::from_array(shared_key))
    {
        Ok(_data) => Err(BitcoinError::UpgradeUnnecessary),
        Err(_err) => {
            // If there's a deserialization error, attempt to recover just the mnemnonic.
            let recovered_wallet_data = EncryptedWalletDataV04::decrypt_owned(
                &encrypted_message,
                &SharedKey::from_array(shared_key),
            )?;

            // println!("Recovered wallet data: {recovered_wallet_data:?}"); // Keep commented out for security
            // todo!("Add later version migrations here");

            let upgraded_descriptor = encrypt_wallet(
                &SecretString(recovered_wallet_data.mnemonic),
                hash,
                seed_password,
            )
            .await?;

            Ok(upgraded_descriptor)
        }
    }
}

pub fn versioned_descriptor(encrypted_message: EncryptedMessage) -> SecretString {
    let mut descriptor_data = DIBA_DESCRIPTOR.to_vec();
    let mut encrypted_descriptors = encrypted_message.serialize();
    descriptor_data.append(&mut encrypted_descriptors);

    let encrypted = SecretString(hex::encode(&descriptor_data));

    descriptor_data.zeroize();
    encrypted_descriptors.zeroize();
    encrypted
}

pub async fn new_wallet(
    hash: &SecretString,
    seed_password: &SecretString,
) -> Result<SecretString, BitcoinError> {
    let mut shared_key: [u8; 32] = hex::decode(&hash.0)?
        .try_into()
        .expect("hash is of fixed size");
    let wallet_data = new_mnemonic(seed_password).await?;
    let encrypted_message = wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let encrypted_descriptors = versioned_descriptor(encrypted_message);

    shared_key.zeroize();
    Ok(encrypted_descriptors)
}

pub async fn encrypt_wallet(
    mnemonic_phrase: &SecretString,
    hash: &SecretString,
    seed_password: &SecretString,
) -> Result<SecretString, BitcoinError> {
    let shared_key: [u8; 32] = hex::decode(&hash.0)?
        .try_into()
        .expect("hash is of fixed size");

    let wallet_data = save_mnemonic(mnemonic_phrase, seed_password).await?;
    let encrypted_message = wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let encrypted_descriptors = versioned_descriptor(encrypted_message);
    Ok(encrypted_descriptors)
}

pub async fn get_wallet_data(
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
) -> Result<WalletData, BitcoinError> {
    info!("get_wallet_data");

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    sync_wallet(&wallet).await?;

    let address = wallet
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .to_string();
    info!(format!("address: {address}"));
    let balance = wallet.lock().await.get_balance()?;
    info!(format!("balance: {balance:?}"));
    let utxos = wallet.lock().await.list_unspent().unwrap_or_default();
    let utxos: Vec<String> = utxos.into_iter().map(|x| x.outpoint.to_string()).collect();
    trace!(format!("unspent: {utxos:#?}"));

    let mut transactions = wallet
        .lock()
        .await
        .list_transactions(false)
        .unwrap_or_default();
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

    trace!(format!("transactions: {transactions:#?}"));

    Ok(WalletData {
        address,
        balance,
        transactions,
        utxos,
    })
}

pub async fn get_new_address(
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
) -> Result<String, BitcoinError> {
    info!("get_new_address");

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    let address = wallet.lock().await.get_address(AddressIndex::New)?;
    info!(format!("address: {address}"));
    Ok(address.to_string())
}

pub async fn validate_address(address: &Address) -> Result<(), BitcoinError> {
    if address.network != *NETWORK.read().await {
        Err(BitcoinError::WrongNetwork)
    } else {
        Ok(())
    }
}

pub async fn send_sats(
    descriptor: &SecretString,
    change_descriptor: &SecretString,
    destination: &str, // bip21 uri or address
    amount: u64,
    fee_rate: Option<f32>,
) -> Result<TransactionDetails, BitcoinError> {
    use payjoin::UriExt;

    let wallet = get_wallet(descriptor, Some(change_descriptor)).await?;
    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let transaction = match payjoin::Uri::try_from(destination) {
        Ok(uri) => {
            let address = uri.address.clone();
            validate_address(&address).await?;
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
            validate_address(&address).await?;
            create_transaction(vec![SatsInvoice { address, amount }], &wallet, fee_rate).await?
        }
    };

    Ok(transaction)
}

pub async fn fund_vault(
    btc_descriptor_xprv: &SecretString,
    btc_change_descriptor_xprv: &SecretString,
    assets_address_1: &str,
    assets_address_2: &str,
    uda_address_1: &str,
    uda_address_2: &str,
    fee_rate: Option<f32>,
) -> Result<FundVaultDetails, BitcoinError> {
    let wallet = get_wallet(btc_descriptor_xprv, Some(btc_change_descriptor_xprv)).await?;
    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let balance = wallet.lock().await.get_balance()?;
    if balance.confirmed < MIN_FUNDS_SATS {
        return Err(BitcoinError::InsufficientFundSats(MIN_FUNDS_SATS));
    };

    let assets_address_1 = Address::from_str(assets_address_1)?;
    let assets_address_2 = Address::from_str(assets_address_2)?;
    let uda_address_1 = Address::from_str(uda_address_1)?;
    let uda_address_2 = Address::from_str(uda_address_2)?;

    let mut rng = StdRng::from_entropy();

    let asset_invoice_1 = SatsInvoice {
        address: assets_address_1,
        amount: rng.gen_range(600..1500),
    };
    let asset_invoice_2 = SatsInvoice {
        address: assets_address_2,
        amount: rng.gen_range(600..1500),
    };
    let uda_invoice_1 = SatsInvoice {
        address: uda_address_1,
        amount: rng.gen_range(600..1500),
    };
    let uda_invoice_2 = SatsInvoice {
        address: uda_address_2,
        amount: rng.gen_range(600..1500),
    };

    let asset_tx_details = create_transaction(
        vec![
            asset_invoice_1,
            asset_invoice_2,
            uda_invoice_1,
            uda_invoice_2,
        ],
        &wallet,
        fee_rate,
    )
    .await?;

    let asset_txid = asset_tx_details.txid;

    info!(format!("asset txid: {asset_txid}"));

    let asset_outputs: Vec<String> = asset_tx_details
        .transaction
        .expect("asset tx should exist but doesn't")
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{asset_txid}:{i}"))
        .collect();

    Ok(FundVaultDetails {
        assets_output: Some(asset_outputs[0].to_owned()),
        assets_change_output: Some(asset_outputs[1].to_owned()),
        udas_output: Some(asset_outputs[2].to_owned()),
        udas_change_output: Some(asset_outputs[3].to_owned()),
        is_funded: true,
    })
}

fn utxo_string(utxo: &LocalUtxo) -> String {
    utxo.outpoint.to_string()
}

pub async fn get_assets_vault(
    rgb_assets_descriptor_xpub: &SecretString,
    rgb_udas_descriptor_xpub: &SecretString,
) -> Result<FundVaultDetails, BitcoinError> {
    let assets_wallet = get_wallet(rgb_assets_descriptor_xpub, None).await?;
    let udas_wallet = get_wallet(rgb_udas_descriptor_xpub, None).await?;

    let assets_utxos = assets_wallet.lock().await.list_unspent()?;
    let uda_utxos = udas_wallet.lock().await.list_unspent()?;

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

pub async fn sign_psbt_file(request: SignPsbtRequest) -> Result<SignPsbtResponse, BitcoinError> {
    let SignPsbtRequest { psbt, descriptors } = request;

    let original_psbt = Psbt::from_str(&psbt)?;
    let final_psbt = PartiallySignedTransaction::from(original_psbt);

    let mut wallets = vec![];
    for descriptor in descriptors {
        let wallet = get_wallet(&descriptor, None).await?;
        wallets.push(wallet);
    }

    let sign = sign_psbt_with_multiple_wallets(wallets, final_psbt).await?;
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

pub async fn drain_wallet(
    destination: &str,
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
    fee_rate: Option<f32>,
) -> Result<TransactionDetails, BitcoinError> {
    let address = Address::from_str(destination)?;
    validate_address(&address).await?;
    debug!(format!("Create drain wallet tx to: {address:#?}"));

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    sync_wallet(&wallet).await?;

    let fee_rate = fee_rate.map(FeeRate::from_sat_per_vb);

    let (mut psbt, details) = {
        let locked_wallet = wallet.lock().await;
        let mut builder = locked_wallet.build_tx();
        if let Some(fee_rate) = fee_rate {
            builder.fee_rate(fee_rate);
        }
        builder.drain_wallet();
        builder.drain_to(address.script_pubkey());
        builder.finish()?
    };

    debug!("Signing PSBT...");
    let finalized = wallet
        .lock()
        .await
        .sign(&mut psbt, SignOptions::default())?;

    if !finalized {
        return Err(BitcoinError::DrainWalletUnfinalizedPsbt);
    }
    debug!(format!("Finalized: {finalized}"));

    let blockchain = get_blockchain().await;
    let tx = psbt.extract_tx();
    blockchain.broadcast(&tx).await?;
    let tx = blockchain.get_tx(&details.txid).await?;

    if let Some(transaction) = tx.clone() {
        let sent = transaction
            .output
            .iter()
            .fold(0, |sum, output| output.value + sum);

        let details = TransactionDetails {
            transaction: tx,
            txid: transaction.txid(),
            received: 0,
            sent,
            fee: details.fee,
            confirmation_time: None,
        };

        info!(format!(
            "Drain wallet transaction submitted with details: {details:#?}"
        ));

        Ok(details)
    } else {
        Err(BitcoinError::DrainWalletNoTxDetails)
    }
}
