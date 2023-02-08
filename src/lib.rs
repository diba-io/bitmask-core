#[macro_use]
#[cfg(not(target_arch = "wasm32"))]
extern crate amplify;

use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Result;
use bdk::{wallet::AddressIndex, FeeRate, LocalUtxo};
#[cfg(not(target_arch = "wasm32"))]
use bitcoin::consensus::serialize as serialize_psbt; // Desktop
use bitcoin::{util::address::Address, OutPoint, Transaction}; // Shared
use bitcoin_hashes::{sha256, Hash};
use serde::{Deserialize, Serialize};
use serde_encrypt::{
    serialize::impls::BincodeSerializer, shared_key::SharedKey, traits::SerdeEncryptSharedKey,
    AsSharedKey, EncryptedMessage,
};
use tokio::try_join;

pub mod data;
pub mod operations;
pub mod util;
#[cfg(target_arch = "wasm32")]
pub mod web;

// Desktop
#[cfg(not(target_arch = "wasm32"))]
pub use crate::{
    data::structs::{AcceptResponse, AssetResponse, FinalizeTransfer, TransfersResponse},
    operations::rgb::{
        self, blind_utxo, get_asset_by_genesis, get_assets, issue_asset, transfer_asset,
        validate_transfer,
    },
};
// Web
#[cfg(target_arch = "wasm32")]
pub use crate::{
    data::structs::{
        AcceptLambdaResponse, AcceptRequest, AssetRequest, BlindRequest, BlindResponse,
        IssueRequest,
    },
    util::post_json,
};
// Shared
pub use crate::{
    data::{
        constants::{get_endpoint, get_network, switch_host, switch_network},
        structs::{
            BlindingUtxo, EncryptedWalletData, FullUtxo, FundVaultDetails, SatsInvoice, ThinAsset,
            TransferResult, TransfersRequest, TransfersSerializeResponse, WalletData,
            WalletTransaction,
        },
    },
    operations::{
        bitcoin::{
            create_payjoin, create_transaction, dust_tx, get_wallet, new_mnemonic, save_mnemonic,
            sign_psbt, synchronize_wallet,
        },
        lightning,
    },
};

impl SerdeEncryptSharedKey for EncryptedWalletData {
    type S = BincodeSerializer<Self>; // you can specify serializer implementation (or implement it by yourself).
}

pub fn get_encrypted_wallet(
    password: &str,
    encrypted_descriptors: &str,
) -> Result<EncryptedWalletData> {
    // read hash digest and consume hasher
    let hash = sha256::Hash::hash(password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();
    let encrypted_descriptors: Vec<u8> = hex::decode(encrypted_descriptors)?;
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors)?;
    Ok(EncryptedWalletData::decrypt_owned(
        &encrypted_message,
        &SharedKey::from_array(shared_key),
    )?)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MnemonicSeedData {
    pub mnemonic: String,
    pub serialized_encrypted_message: String,
}

// TODO: should probably be called "new_mnemonic_seed"
pub fn get_mnemonic_seed(
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let encrypted_wallet_data = new_mnemonic(seed_password)?;
    let encrypted_message = encrypted_wallet_data.encrypt(&SharedKey::from_array(shared_key))?;
    let serialized_encrypted_message = hex::encode(encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: encrypted_wallet_data.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub fn save_mnemonic_seed(
    mnemonic_phrase: &str,
    encryption_password: &str,
    seed_password: &str,
) -> Result<MnemonicSeedData> {
    let hash = sha256::Hash::hash(encryption_password.as_bytes());
    let shared_key: [u8; 32] = hash.into_inner();

    let vault_data = save_mnemonic(mnemonic_phrase, seed_password)?;
    let encrypted_message = vault_data.encrypt(&SharedKey::from_array(shared_key))?;
    let serialized_encrypted_message = hex::encode(encrypted_message.serialize());
    let mnemonic_seed_data = MnemonicSeedData {
        mnemonic: vault_data.mnemonic,
        serialized_encrypted_message,
    };

    Ok(mnemonic_seed_data)
}

pub async fn get_wallet_data(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<WalletData> {
    info!("get_wallet_data");
    info!("descriptor:", &descriptor);
    info!("change_descriptor:", format!("{:?}", &change_descriptor));

    let wallet = get_wallet(descriptor, change_descriptor)?;
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

    let wallet = get_wallet(descriptor, change_descriptor)?;
    synchronize_wallet(&wallet).await?;
    let address = wallet.get_address(AddressIndex::New)?.to_string();
    info!(format!("address: {address}"));
    Ok(address)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_assets(contract: &str) -> Result<Vec<AssetResponse>> {
    info!("list_assets");
    let assets = get_assets(contract)?;
    info!(format!("get assets: {assets:#?}"));
    Ok(assets)
}

// TODO: web list_assets

#[derive(Serialize, Deserialize)]
pub struct CreateAssetResult {
    pub genesis: String,   // in bech32m encoding
    pub id: String,        // contract ID
    pub asset_id: String,  // asset ID
    pub schema_id: String, // schema ID (i.e., RGB20)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_asset(
    ticker: &str,
    name: &str,
    precision: u8,
    supply: u64,
    utxo: &str,
) -> Result<CreateAssetResult> {
    let utxo = OutPoint::from_str(utxo)?;
    let contract = issue_asset(ticker, name, precision, supply, utxo)?;
    let genesis = contract.to_string();
    let id = contract.id().to_string();
    let asset_id = contract.contract_id().to_string();
    let schema_id = contract.schema_id().to_string();

    Ok(CreateAssetResult {
        genesis,
        id,
        asset_id,
        schema_id,
    })
}

#[cfg(target_arch = "wasm32")]
pub async fn create_asset(
    ticker: &str,
    name: &str,
    precision: u8,
    supply: u64,
    utxo: &str,
) -> Result<CreateAssetResult> {
    let endpoint = &get_endpoint("issue").await;
    let body = IssueRequest {
        ticker: ticker.to_owned(),
        name: name.to_owned(),
        description: "TODO".to_owned(),
        precision,
        supply,
        utxo: utxo.to_owned(),
    };
    let (issue_res, status) = post_json(endpoint, &body).await?;
    if status != 200 {
        return Err(anyhow!("Error calling {endpoint}"));
    }
    let CreateAssetResult {
        genesis,
        id,
        asset_id,
        schema_id,
    } = serde_json::from_str(&issue_res)?;

    Ok(CreateAssetResult {
        genesis,
        id,
        asset_id,
        schema_id,
    })
}

pub async fn get_utxos(
    descriptor: &str,
    change_descriptor: Option<String>,
) -> Result<Vec<LocalUtxo>> {
    let rgb_wallet = get_wallet(descriptor, change_descriptor)?;
    synchronize_wallet(&rgb_wallet).await?;
    let utxos = rgb_wallet.list_unspent()?;

    Ok(utxos)
}

pub fn parse_outpoints(outpoints: Vec<String>) -> Result<Vec<OutPoint>> {
    outpoints
        .into_iter()
        .map(|outpoint| OutPoint::from_str(&outpoint).map_err(|e| anyhow!(e)))
        .collect()
}

pub fn utxos_to_outpoints(utxos: Vec<LocalUtxo>) -> Vec<String> {
    utxos
        .into_iter()
        .map(|utxo| utxo.outpoint.to_string())
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn import_asset(asset: &str, utxos: Vec<String>) -> Result<ThinAsset> {
    let utxos = parse_outpoints(utxos)?;

    match asset.as_bytes() {
        #[allow(unreachable_code)]
        [b'r', b'g', b'b', b'1', ..] => Ok(todo!(
            "asset persistence for asset_import not yet implemented"
        )),
        [b'r', b'g', b'b', b'c', b'1', ..] => {
            info!("Getting asset by contract genesis:", asset);
            get_asset_by_genesis(asset, &utxos)
        }
        _ => Err(anyhow!("Asset did not match expected format")),
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn import_asset(asset: &str, utxo: &str) -> Result<ThinAsset> {
    info!("Getting asset:", asset);

    let endpoint = &get_endpoint("import").await;
    let body = AssetRequest {
        asset: asset.to_owned(),
        utxos: vec![utxo.to_owned()],
    };
    let (asset_res, status) = post_json(endpoint, &body).await?;
    if status != 200 {
        return Err(anyhow!("Error calling {endpoint}"));
    }
    let ThinAsset {
        id,
        ticker,
        name,
        description,
        allocations,
        balance,
        genesis,
    } = serde_json::from_str(&asset_res)?;

    Ok(ThinAsset {
        id,
        ticker,
        name,
        description,
        allocations,
        balance,
        genesis,
    })
}

#[derive(Serialize, Deserialize)]
struct TransactionData {
    blinding: String,
    utxo: OutPoint,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_blinded_utxo(utxo_string: &Option<String>) -> Result<BlindingUtxo> {
    if let Some(utxo_string) = utxo_string {
        let utxo = OutPoint::from_str(utxo_string)?;

        let blind = blind_utxo(utxo)?;

        let blinding_utxo = BlindingUtxo {
            conceal: blind.conceal,
            blinding: blind.blinding,
            utxo,
        };

        Ok(blinding_utxo)
    } else {
        Err(anyhow!("No utxo string passed"))
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn get_blinded_utxo(utxo_string: &str) -> Result<BlindingUtxo> {
    let utxo = OutPoint::from_str(utxo_string)?;

    let endpoint = &get_endpoint("blind").await;
    let body = BlindRequest {
        utxo: utxo.to_string(),
    };
    let (blind_res, status) = post_json(endpoint, &body).await?;
    if status != 200 {
        return Err(anyhow!("Error calling {endpoint}"));
    }
    let BlindResponse { conceal, blinding } = serde_json::from_str(&blind_res)?;
    let blinding_utxo = BlindingUtxo {
        conceal,
        blinding,
        utxo,
    };

    Ok(blinding_utxo)
}

pub async fn send_sats(
    descriptor: &str,
    change_descriptor: &str,
    destination: &str, // bip21 uri or address
    amount: u64,
    fee_rate: Option<f32>,
) -> Result<Transaction> {
    use payjoin::UriExt;

    let wallet = get_wallet(descriptor, Some(change_descriptor.to_owned()))?;
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
    )?;
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

    let asset_txid = asset_tx_details.txid();
    let asset_outputs: Vec<String> = asset_tx_details
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{asset_txid}:{i}"))
        .collect();

    let uda_txid = uda_tx_details.txid();
    let uda_outputs: Vec<String> = uda_tx_details
        .output
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{uda_txid}:{i}"))
        .collect();

    Ok(FundVaultDetails {
        assets_spend: Some(asset_outputs[0].to_owned()),
        assets_receive: Some(asset_outputs[1].to_owned()),
        udas_spend: Some(uda_outputs[0].to_owned()),
        udas_receive: Some(uda_outputs[1].to_owned()),
    })
}

fn utxo_string(utxo: &LocalUtxo) -> String {
    utxo.outpoint.to_string()
}

pub async fn get_assets_vault(
    rgb_assets_descriptor_xpub: &str,
    rgb_udas_descriptor_xpub: &str,
) -> Result<FundVaultDetails> {
    let assets_wallet = get_wallet(rgb_assets_descriptor_xpub, None)?;
    let udas_wallet = get_wallet(rgb_udas_descriptor_xpub, None)?;

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

    let assets_receive = assets_utxos.pop();
    let assets_spend = assets_utxos.pop();
    let udas_receive = uda_utxos.pop();
    let udas_spend = uda_utxos.pop();

    Ok(FundVaultDetails {
        assets_spend,
        assets_receive,
        udas_spend,
        udas_receive,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn transfer_assets(transfers: TransfersRequest) -> Result<TransfersSerializeResponse> {
    use rgb_std::{Contract, StateTransfer};
    use strict_encoding::strict_serialize;

    use crate::data::structs::{BlindedOrNotOutpoint, ChangeTansfer, DeclareRequest};

    let resp = transfer_asset(transfers).await?;

    let psbt = serialize_psbt(&resp.psbt);
    let psbt = base64::encode(&psbt);
    let disclosure = serde_json::to_string(&resp.disclosure)?;

    let mut transfers = vec![];
    let mut change_transfers = vec![];

    // Retrieve consignment information + transition state
    let state_transfers: Vec<StateTransfer> =
        resp.transfers.clone().into_iter().map(|(f, _)| f).collect();

    for (index, asset_transfer_info) in resp.transaction_info.iter().enumerate() {
        let consig = Contract::from_str(&asset_transfer_info.consignment.clone())?;
        let state_transfer = state_transfers
            .clone()
            .into_iter()
            .find(|st| st.contract_id() == consig.contract_id());

        let state_serialize = strict_serialize(&state_transfer.unwrap())
            .expect("Consignment information must be valid");
        let state_serialize = util::bech32m_zip_encode("rgbc", &state_serialize)
            .expect("Strict encoded information must be a valid consignment");

        transfers.push(FinalizeTransfer {
            consignment: state_serialize,
            asset: asset_transfer_info.asset_contract.clone(),
            beneficiaries: asset_transfer_info
                .beneficiaries
                .iter()
                .map(|info| {
                    let parts: Vec<&str> = info.split('@').collect();
                    let balance = parts[0].parse::<u64>().unwrap();
                    let outpoint = parts[1].to_string();
                    BlindedOrNotOutpoint { outpoint, balance }
                })
                .collect(),
            previous_utxo: resp.origin[index].outpoint.to_string(),
        });
        change_transfers.push(ChangeTansfer {
            asset: asset_transfer_info.asset_contract.clone(),
            change: BlindedOrNotOutpoint {
                outpoint: asset_transfer_info.change_utxo.clone(),
                balance: asset_transfer_info.change,
            },
            previous_utxo: resp.origin[index].outpoint.to_string(),
        });
    }

    Ok(TransfersSerializeResponse {
        psbt,
        declare: DeclareRequest {
            change_transfers,
            transfers,
            disclosure,
        },
    })
}

#[cfg(target_arch = "wasm32")]
pub async fn transfer_assets(transfers: TransfersRequest) -> Result<TransfersSerializeResponse> {
    let endpoint = &get_endpoint("send").await;
    let body = transfers;

    let (transfer_res, status) = post_json(endpoint, &body).await?;
    if status != 200 {
        return Err(anyhow!("Error calling {endpoint}"));
    }

    let transfer_res: TransfersSerializeResponse = serde_json::from_str(&transfer_res)?;

    let _declare = async {
        let declare_endpoint = &get_endpoint("declare").await;
        let (_, status) = post_json(declare_endpoint, &transfer_res.declare).await?;
        if status != 200 {
            return Err(anyhow!("Error calling {declare_endpoint}"));
        }
        Ok(status)
    }
    .await?;

    Ok(transfer_res)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn accept_transfer(
    consignment: &str,
    blinding_factor: &str,
    outpoint: &str,
) -> Result<AcceptResponse> {
    let (id, info, valid) = rgb::accept_transfer(consignment, blinding_factor, outpoint).await?;
    if valid {
        info!("Transaction accepted");
        Ok(AcceptResponse { id, info, valid })
    } else {
        Err(anyhow!("Incorrect seals. id: {} stratus: {}", id, info))
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn sign_psbt_web(rgb_descriptor_xprv: &str, psbt: &str) -> Result<Transaction> {
    use bitcoin::psbt::PartiallySignedTransaction;

    let wallet = get_wallet(rgb_descriptor_xprv, None)?;
    synchronize_wallet(&wallet).await?;

    let psbt = PartiallySignedTransaction::from_str(psbt)?;
    let transaction = sign_psbt(&wallet, psbt).await?;

    Ok(transaction)
}

#[cfg(target_arch = "wasm32")]
pub async fn accept_transfer(
    consignment: &str,
    blinding_factor: &str,
    outpoint: &str,
    blinded: &str,
) -> Result<AcceptLambdaResponse> {
    let endpoint = &get_endpoint("accept").await;
    let body = AcceptRequest {
        consignment: consignment.to_owned(),
        blinding_factor: blinding_factor.to_owned(),
        outpoint: outpoint.to_owned(),
        blinded: blinded.to_owned(),
    };
    let (transfer_res, status) = post_json(endpoint, &body).await?;
    if status != 200 {
        return Err(anyhow!("Error calling {endpoint}"));
    }
    Ok(serde_json::from_str(&transfer_res)?)
}
