#![cfg(not(target_arch = "wasm32"))]
use std::{env, process::Stdio};

use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, get_wallet_data, save_mnemonic, synchronize_wallet},
    rgb::{
        create_invoice, create_psbt, create_watcher, import, issue_contract, transfer_asset,
        watcher_details, watcher_next_address, watcher_next_utxo,
    },
    structs::{
        AllocationDetail, ContractType, EncryptedWalletData, ImportRequest, ImportResponse,
        InvoiceRequest, InvoiceResponse, IssueRequest, IssueResponse, PsbtRequest, PsbtResponse,
        RgbTransferRequest, RgbTransferResponse, WatcherRequest,
    },
};
use tokio::process::Command;

pub const ISSUER_MNEMONIC: &str =
    "ordinary crucial edit settle pencil lion appear unlock left fly century license";

#[allow(dead_code)]
pub const OWNER_MNEMONIC: &str =
    "apology pull visa moon retreat spell elite extend secret region fly diary";

#[allow(dead_code)]
pub async fn start_node() {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/tests/scripts/startup_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("oh no!");
}

pub async fn send_some_coins(address: &str, amount: &str) {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/tests/scripts/send_coins.sh", path);
    Command::new("bash")
        .arg(full_file)
        .args([address, amount])
        .stdout(Stdio::null())
        // .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("oh no!");
}

#[allow(dead_code)]
pub async fn stop_node() {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/tests/scripts/stop_node.sh", path);
    Command::new("bash")
        .arg(full_file)
        .stdout(Stdio::null())
        // .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .await
        .expect("oh no!");
}

pub async fn setup_regtest(force: bool, mnemonic: Option<&str>) {
    if force {
        // Restart Nodes
        start_node().await;
    }
    if let Some(words) = mnemonic {
        let seed_password = "";
        let vault_data = bitmask_core::bitcoin::save_mnemonic(words, seed_password)
            .await
            .expect("invalid mnemonic");

        // Send Coins to RGB Wallet
        let fungible_snapshot =
            get_wallet_data(&vault_data.public.rgb_assets_descriptor_xpub, None)
                .await
                .expect("invalid wallet snapshot");
        send_some_coins(&fungible_snapshot.address, "0.1").await;
    };
}

#[allow(dead_code)]
pub async fn shutdown_regtest(force: bool) -> anyhow::Result<()> {
    if force {
        // Destroy Nodes
        stop_node().await;
    }
    Ok(())
}

pub async fn issuer_issue_contract(
    iface: &str,
    force: bool,
) -> Result<IssueResponse, anyhow::Error> {
    setup_regtest(force, None).await;
    let issuer_keys = save_mnemonic(ISSUER_MNEMONIC, "").await?;
    let watcher_name = "default";
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub,
    };

    // Create Watcher
    let sk = issuer_keys.private.nostr_prv;
    let resp = create_watcher(&sk, create_watch_req).await;
    assert!(resp.is_ok());

    let next_address = watcher_next_address(&sk, watcher_name, iface).await?;
    send_some_coins(&next_address.address, "0.1").await;

    let next_utxo = watcher_next_utxo(&sk, watcher_name, iface).await?;

    let issue_utxo = next_utxo.utxo;
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let request = IssueRequest {
        ticker: "DIBA".to_string(),
        name: "DIBA".to_string(),
        description: "DIBA".to_string(),
        precision: 2,
        supply: 5,
        seal: issue_seal.to_owned(),
        iface: iface.to_string(),
    };

    issue_contract(&sk, request).await
}

pub async fn import_new_contract(
    issuer_resp: IssueResponse,
) -> Result<ImportResponse, anyhow::Error> {
    let owner_keys = save_mnemonic(OWNER_MNEMONIC, "").await?;

    // Create Watcher
    let sk = owner_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: "default".to_owned(),
        xpub: owner_keys.public.watcher_xpub,
    };

    let resp = create_watcher(&sk, create_watch_req).await;
    assert!(resp.is_ok());

    let contract_type = match issuer_resp.iface.as_str() {
        "RGB20" => ContractType::RGB20,
        "RGB21" => ContractType::RGB21,
        _ => ContractType::Contract,
    };

    // Import Contract
    let import_req = ImportRequest {
        import: contract_type,
        data: issuer_resp.contract.strict,
    };

    let resp = import(&sk, import_req).await;
    assert!(resp.is_ok());
    resp
}

pub async fn create_new_invoice(
    issuer_resp: IssueResponse,
) -> Result<InvoiceResponse, anyhow::Error> {
    let owner_keys = save_mnemonic(OWNER_MNEMONIC, "").await?;
    let owner_vault = get_wallet(&owner_keys.public.rgb_assets_descriptor_xpub, None).await?;

    // Create Watcher
    let sk = owner_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: "default".to_owned(),
        xpub: owner_keys.public.watcher_xpub,
    };

    let resp = create_watcher(&sk, create_watch_req).await;
    assert!(resp.is_ok());

    let contract_type = match issuer_resp.iface.as_str() {
        "RGB20" => ContractType::RGB20,
        "RGB21" => ContractType::RGB21,
        _ => ContractType::Contract,
    };

    // Import Contract
    let import_req = ImportRequest {
        import: contract_type,
        data: issuer_resp.contract.legacy,
    };

    let resp = import(&sk, import_req).await;
    assert!(resp.is_ok());

    // Create Invoice
    let owner_address = &owner_vault
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(owner_address, "0.1").await;
    synchronize_wallet(&owner_vault).await?;

    let beneficiary_utxo = owner_vault.list_unspent()?;
    let beneficiary_utxo = beneficiary_utxo.first().unwrap();
    let seal = beneficiary_utxo.outpoint.to_string();
    let seal = format!("tapret1st:{seal}");

    let invoice_req = InvoiceRequest {
        contract_id: issuer_resp.contract_id,
        iface: issuer_resp.iface,
        amount: 1,
        seal,
    };

    create_invoice(&sk, invoice_req).await
}

pub async fn create_new_psbt(
    issuer_keys: EncryptedWalletData,
    issuer_resp: IssueResponse,
) -> Result<PsbtResponse, anyhow::Error> {
    // Get Allocations
    let watcher_name = "default";
    let sk = issuer_keys.private.nostr_prv;
    let resp = watcher_details(&sk, watcher_name).await;
    assert!(resp.is_ok());

    let mut asset_utxo = String::new();
    let mut asset_utxo_terminal = String::new();
    let watcher_details = resp?;
    for contract_allocations in watcher_details.contracts {
        let allocations: Vec<AllocationDetail> = contract_allocations
            .allocations
            .into_iter()
            .filter(|a| a.is_mine && a.utxo == issuer_resp.issue_utxo)
            .collect();

        if let Some(allocation) = allocations.into_iter().next() {
            asset_utxo = allocation.utxo.to_owned();
            asset_utxo_terminal = allocation.derivation.to_owned();
        }
    }

    assert_eq!(asset_utxo, issuer_resp.issue_utxo);
    let req = PsbtRequest {
        descriptor_pub: issuer_keys.public.rgb_assets_descriptor_xpub,
        asset_utxo: asset_utxo.to_string(),
        asset_utxo_terminal: asset_utxo_terminal.to_string(),
        change_index: None,
        bitcoin_changes: vec![],
        fee: None,
        input_tweak: None,
    };

    create_psbt(&sk, req).await
}

pub async fn create_new_transfer(
    issuer_keys: EncryptedWalletData,
    owner_resp: InvoiceResponse,
    psbt_resp: PsbtResponse,
) -> Result<RgbTransferResponse, anyhow::Error> {
    // Get Allocations
    let transfer_req = RgbTransferRequest {
        psbt: psbt_resp.psbt,
        rgb_invoice: owner_resp.invoice,
    };

    let sk = issuer_keys.private.nostr_prv;

    transfer_asset(&sk, transfer_req).await
}
