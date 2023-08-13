#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use bitmask_core::{
    bitcoin::{save_mnemonic, sign_psbt_file},
    rgb::{create_watcher, list_transfers, remove_transfer, save_transfer, watcher_next_address},
    structs::{
        DecryptedWalletData, RgbRemoveTransferRequest, RgbSaveTransferRequest, SecretString,
        SignPsbtRequest, TransferType, TxStatus, WatcherRequest,
    },
};

use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, issuer_issue_contract_v2,
    send_some_coins, UtxoFilter, ISSUER_MNEMONIC, OWNER_MNEMONIC,
};

#[tokio::test]
pub async fn allow_save_read_remove_transfers() -> Result<()> {
    // 0. Retrieve all keys
    let issuer_keys: DecryptedWalletData = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = &save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    // 1. Create All Watchers
    let watcher_name = "default";
    let issuer_sk = issuer_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: issuer_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&issuer_sk, create_watch_req.clone()).await?;

    let owner_sk = owner_keys.private.nostr_prv.to_string();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        xpub: owner_keys.public.watcher_xpub.clone(),
        force: true,
    };
    create_watcher(&owner_sk, create_watch_req.clone()).await?;

    // 2. Issuer Contract
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB20",
        5,
        false,
        true,
        None,
        Some("0.1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(10000000)),
        None,
    )
    .await?;
    let issuer_resp = &issuer_resp[0];

    // 3. Owner Create Invoice
    let owner_invoice = &create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        2,
        owner_keys.clone(),
        None,
        Some(issuer_resp.clone().contract.strict),
    )
    .await?;

    // 4. Create First Transfer
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = &create_new_transfer(
        issuer_keys.clone(),
        owner_invoice.clone(),
        psbt_resp.clone(),
    )
    .await?;

    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt.clone(),
        descriptors: vec![SecretString(
            issuer_keys.private.rgb_assets_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_psbt_file(request).await;
    assert!(resp.is_ok());

    // 5. Save Consig (Owner Side)
    let transfer = transfer_resp.clone();
    let all_sks = [owner_sk.clone()];
    for sk in all_sks {
        let request = RgbSaveTransferRequest {
            contract_id: issuer_resp.contract_id.clone(),
            consignment: transfer.consig.clone(),
        };
        let save_resp = save_transfer(&sk, request).await;
        assert!(save_resp.is_ok());
    }

    // 6. Check Consig Status (Both Sides)
    let contract_id = issuer_resp.contract_id.clone();
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let is_issuer = sk == issuer_sk;

        let list_resp = list_transfers(&sk, contract_id.clone()).await;
        assert!(list_resp.is_ok());

        let list_resp = list_resp?;
        if let Some(consig_status) = list_resp
            .transfers
            .into_iter()
            .find(|x| x.consig_id == transfer.consig_id)
        {
            matches!(consig_status.status, TxStatus::Mempool);

            if is_issuer {
                assert_eq!(consig_status.ty, TransferType::Sended);
            } else {
                assert_eq!(consig_status.ty, TransferType::Received);
            }
        }
    }

    // 7. Check Consig Status After Block (Both Sides)
    let address = watcher_next_address(&owner_sk, watcher_name, "RGB20").await?;
    send_some_coins(&address.address, "0.1").await;

    let contract_id = issuer_resp.contract_id.clone();
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let list_resp = list_transfers(&sk, contract_id.clone()).await;
        assert!(list_resp.is_ok());

        let list_resp = list_resp?;
        if let Some(consig_status) = list_resp
            .transfers
            .into_iter()
            .find(|x| x.consig_id == transfer.consig_id)
        {
            matches!(consig_status.status, TxStatus::Block(_));
        }
    }

    // 8. Remove Consig (Both Sides)
    let contract_id = issuer_resp.contract_id.clone();
    let all_sks = [issuer_sk.clone(), owner_sk.clone()];
    for sk in all_sks {
        let req = RgbRemoveTransferRequest {
            contract_id: contract_id.clone(),
            consig_ids: vec![transfer.consig_id.clone()],
        };
        let list_resp = remove_transfer(&sk, req).await;
        assert!(list_resp.is_ok());

        let list_resp = list_transfers(&sk, contract_id.clone()).await;
        assert!(list_resp.is_ok());

        let list_resp = list_resp?;
        assert_eq!(list_resp.transfers.len(), 0);
    }

    Ok(())
}
