#![cfg(not(target_arch = "wasm32"))]
use std::{collections::HashMap, env, process::Stdio};

use bdk::wallet::AddressIndex;
use bitmask_core::{
    bitcoin::{get_wallet, get_wallet_data, save_mnemonic, sync_wallet},
    rgb::{
        create_invoice, create_psbt, create_watcher, import, issue_contract, transfer_asset,
        watcher_details, watcher_next_address, watcher_next_utxo,
    },
    structs::{
        AllocationDetail, AssetType, ContractResponse, DecryptedWalletData, ImportRequest,
        InvoiceRequest, InvoiceResponse, IssueMetaRequest, IssueMetadata, IssueRequest,
        IssueResponse, MediaInfo, NewCollectible, PsbtInputRequest, PsbtRequest, PsbtResponse,
        RgbTransferRequest, RgbTransferResponse, SecretString, WatcherRequest,
    },
};
use tokio::process::Command;

pub const ISSUER_MNEMONIC: &str =
    "ordinary crucial edit settle pencil lion appear unlock left fly century license";

pub const OWNER_MNEMONIC: &str =
    "apology pull visa moon retreat spell elite extend secret region fly diary";

pub const ANOTHER_OWNER_MNEMONIC: &str =
    "circle hold drift unable own laptop age relax degree next alone stage";

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
        let vault_data = bitmask_core::bitcoin::save_mnemonic(
            &SecretString(words.to_string()),
            &SecretString(seed_password.to_string()),
        )
        .await
        .expect("invalid mnemonic");

        // Send Coins to RGB Wallet
        let fungible_snapshot = get_wallet_data(
            &SecretString(vault_data.public.rgb_assets_descriptor_xpub.clone()),
            None,
        )
        .await
        .expect("invalid wallet snapshot");
        send_some_coins(&fungible_snapshot.address, "0.1").await;

        // Send Coins to RGB Wallet
        let uda_snapshot = get_wallet_data(
            &SecretString(vault_data.public.rgb_udas_descriptor_xpub.clone()),
            None,
        )
        .await
        .expect("invalid wallet snapshot");
        send_some_coins(&uda_snapshot.address, "0.1").await;
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

#[allow(dead_code)]
pub async fn send_coins(iface: &str, _watcher_pub: &str) -> anyhow::Result<()> {
    let watcher_name = "default";
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Send Coins
    let sk = &issuer_keys.private.nostr_prv;
    let next_address = watcher_next_address(sk, watcher_name, iface).await?;
    send_some_coins(&next_address.address, "0.01").await;
    Ok(())
}

pub async fn issuer_issue_contract(
    iface: &str,
    supply: u64,
    force: bool,
    send_coins: bool,
    meta: Option<IssueMetaRequest>,
) -> Result<IssueResponse, anyhow::Error> {
    setup_regtest(force, None).await;
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let watcher_name = "default";

    // Create Watcher
    let sk = &issuer_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: send_coins,
    };

    create_watcher(sk, create_watch_req.clone()).await?;

    if send_coins {
        let next_address = watcher_next_address(sk, watcher_name, iface).await?;
        send_some_coins(&next_address.address, "0.01").await;
    }

    let mut next_utxo = watcher_next_utxo(sk, watcher_name, iface).await?;
    if next_utxo.utxo.is_empty() {
        let next_address = watcher_next_address(sk, watcher_name, iface).await?;
        send_some_coins(&next_address.address, "0.01").await;

        next_utxo = watcher_next_utxo(sk, watcher_name, iface).await?;
    }

    let issue_utxo = next_utxo.utxo;
    let issue_seal = format!("tapret1st:{issue_utxo}");
    let request = IssueRequest {
        ticker: "DIBA".to_string(),
        name: "DIBA".to_string(),
        description: "DIBA".to_string(),
        precision: 2,
        supply,
        seal: issue_seal.to_owned(),
        iface: iface.to_string(),
        meta,
    };

    issue_contract(sk, request).await
}

pub async fn import_new_contract(
    issuer_resp: IssueResponse,
) -> Result<ContractResponse, anyhow::Error> {
    let owner_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // Create Watcher
    let sk = owner_keys.private.nostr_prv.clone();
    let create_watch_req = WatcherRequest {
        name: "default".to_owned(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: true,
    };

    let resp = create_watcher(&sk, create_watch_req).await;
    assert!(resp.is_ok());

    let contract_type = match issuer_resp.iface.as_str() {
        "RGB20" => AssetType::RGB20,
        "RGB21" => AssetType::RGB21,
        _ => AssetType::Contract,
    };

    // Import Contract
    let import_req = ImportRequest {
        import: contract_type,
        data: issuer_resp.contract.legacy,
    };

    let resp = import(&sk, import_req).await;
    assert!(resp.is_ok());
    resp
}

pub async fn create_new_invoice(
    contract_id: &str,
    iface: &str,
    amount: u64,
    owner_keys: DecryptedWalletData,
    params: Option<HashMap<String, String>>,
    contract: Option<String>,
) -> Result<InvoiceResponse, anyhow::Error> {
    let descriptor_pub = match iface {
        "RGB20" => owner_keys.public.rgb_assets_descriptor_xpub.clone(),
        "RGB21" => owner_keys.public.rgb_udas_descriptor_xpub.clone(),
        _ => owner_keys.public.rgb_assets_descriptor_xpub.clone(),
    };
    let owner_vault = get_wallet(&SecretString(descriptor_pub), None).await?;

    // Create Watcher
    let sk = owner_keys.private.nostr_prv.clone();
    let contract_type = match iface {
        "RGB20" => AssetType::RGB20,
        "RGB21" => AssetType::RGB21,
        _ => AssetType::Contract,
    };

    if let Some(contract) = contract {
        // Import Contract
        let import_req = ImportRequest {
            import: contract_type,
            data: contract,
        };

        let resp = import(&sk, import_req).await;
        assert!(resp.is_ok());
    }
    // Create Invoice
    let owner_address = &owner_vault
        .lock()
        .await
        .get_address(AddressIndex::LastUnused)?
        .address
        .to_string();

    send_some_coins(owner_address, "0.1").await;
    sync_wallet(&owner_vault).await?;

    let beneficiary_utxo = owner_vault.lock().await.list_unspent()?;
    let beneficiary_utxo = beneficiary_utxo.first().unwrap();
    let seal = beneficiary_utxo.outpoint.to_string();
    let seal = format!("tapret1st:{seal}");

    let params = params.unwrap_or_default();
    let invoice_req = InvoiceRequest {
        contract_id: contract_id.to_owned(),
        iface: iface.to_owned(),
        amount,
        seal,
        params,
    };

    create_invoice(&sk, invoice_req).await
}

pub async fn create_new_psbt(
    contract_id: &str,
    iface: &str,
    owner_utxos: Vec<String>,
    owner_keys: DecryptedWalletData,
) -> Result<PsbtResponse, anyhow::Error> {
    // Get Allocations
    let watcher_name = "default";
    let sk = owner_keys.private.nostr_prv.clone();
    let resp = watcher_details(&sk, watcher_name).await;
    assert!(resp.is_ok());

    let mut inputs = vec![];
    let watcher_details = resp?;
    for contract_allocations in watcher_details
        .contracts
        .into_iter()
        .filter(|x| x.contract_id == contract_id)
    {
        let allocations: Vec<AllocationDetail> = contract_allocations
            .allocations
            .into_iter()
            .filter(|a| a.is_mine && !a.is_spent && owner_utxos.contains(&a.utxo))
            .collect();

        if let Some(allocation) = allocations.into_iter().next() {
            inputs.push(PsbtInputRequest {
                asset_utxo: allocation.utxo.to_owned(),
                asset_utxo_terminal: allocation.derivation,
                tapret: None,
            })
        }
    }

    let descriptor_pub = match iface {
        "RGB20" => owner_keys.public.rgb_assets_descriptor_xpub.clone(),
        "RGB21" => owner_keys.public.rgb_udas_descriptor_xpub.clone(),
        _ => owner_keys.public.rgb_assets_descriptor_xpub.clone(),
    };

    let req = PsbtRequest {
        descriptor_pub: SecretString(descriptor_pub),
        inputs,
        change_index: None,
        bitcoin_changes: vec![],
        fee: None,
    };

    create_psbt(&sk, req).await
}

pub async fn create_new_invoice_v2(
    contract_id: &str,
    iface: &str,
    amount: u64,
    utxo: &str,
    owner_keys: DecryptedWalletData,
    params: Option<HashMap<String, String>>,
    contract: Option<String>,
) -> Result<InvoiceResponse, anyhow::Error> {
    // Create Watcher
    let sk = owner_keys.private.nostr_prv.clone();
    let contract_type = match iface {
        "RGB20" => AssetType::RGB20,
        "RGB21" => AssetType::RGB21,
        _ => AssetType::Contract,
    };

    if let Some(contract) = contract {
        // Import Contract
        let import_req = ImportRequest {
            import: contract_type,
            data: contract,
        };

        let resp = import(&sk, import_req).await;
        assert!(resp.is_ok());
    }

    let seal = format!("tapret1st:{utxo}");

    let params = params.unwrap_or_default();
    let invoice_req = InvoiceRequest {
        contract_id: contract_id.to_owned(),
        iface: iface.to_owned(),
        amount,
        seal,
        params,
    };

    create_invoice(&sk, invoice_req).await
}

pub async fn create_new_psbt_v2(
    iface: &str,
    owner_utxos: Vec<AllocationDetail>,
    owner_keys: DecryptedWalletData,
) -> Result<PsbtResponse, anyhow::Error> {
    // Get Allocations
    let watcher_name = "default";
    let sk = owner_keys.private.nostr_prv.clone();
    let resp = watcher_details(&sk, watcher_name).await;
    assert!(resp.is_ok());

    let inputs = owner_utxos
        .into_iter()
        .map(|x| PsbtInputRequest {
            asset_utxo: x.utxo.to_owned(),
            asset_utxo_terminal: x.derivation,
            tapret: None,
        })
        .collect();

    let descriptor_pub = match iface {
        "RGB20" => owner_keys.public.rgb_assets_descriptor_xpub.clone(),
        "RGB21" => owner_keys.public.rgb_udas_descriptor_xpub.clone(),
        _ => owner_keys.public.rgb_assets_descriptor_xpub.clone(),
    };

    let req = PsbtRequest {
        descriptor_pub: SecretString(descriptor_pub),
        inputs,
        change_index: None,
        bitcoin_changes: vec![],
        fee: None,
    };

    create_psbt(&sk, req).await
}

pub async fn create_new_transfer(
    owner_keys: DecryptedWalletData,
    invoice_resp: InvoiceResponse,
    psbt_resp: PsbtResponse,
) -> Result<RgbTransferResponse, anyhow::Error> {
    // Get Allocations
    let transfer_req = RgbTransferRequest {
        psbt: psbt_resp.psbt,
        rgb_invoice: invoice_resp.invoice,
        terminal: psbt_resp.terminal,
    };

    let sk = owner_keys.private.nostr_prv.clone();
    transfer_asset(&sk, transfer_req).await
}

pub fn get_uda_data() -> IssueMetaRequest {
    IssueMetaRequest::with(IssueMetadata::UDA(vec![MediaInfo {
        ty: "image/png".to_string(),
        source: "https://carbonado.io/diba.png".to_string(),
    }]))
}

pub fn _get_collectible_data() -> IssueMetaRequest {
    IssueMetaRequest::with(IssueMetadata::Collectible(vec![
        NewCollectible {
            ticker: "DIBAA".to_string(),
            name: "DIBAA".to_string(),
            media: vec![MediaInfo {
                ty: "image/png".to_string(),
                source: "https://carbonado.io/diba1.png".to_string(),
            }],
            ..Default::default()
        },
        NewCollectible {
            ticker: "DIBAB".to_string(),
            name: "DIBAB".to_string(),
            media: vec![MediaInfo {
                ty: "image/png".to_string(),
                source: "https://carbonado.io/diba2.png".to_string(),
            }],
            ..Default::default()
        },
    ]))
}
