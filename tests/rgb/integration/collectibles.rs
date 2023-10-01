#![cfg(not(target_arch = "wasm32"))]
// use crate::rgb::integration::utils::{
//     create_new_invoice, create_new_psbt, create_new_transfer, issuer_issue_contract,
//     ISSUER_MNEMONIC,
// };
// use bitmask_core::{
//     bitcoin::{save_mnemonic, sign_and_publish_psbt_file},
//     rgb::accept_transfer,
//     structs::{AcceptRequest, SecretString, SignPsbtRequest},
// };

// #[tokio::test]
// async fn _allow_beneficiary_accept_transfer() -> anyhow::Result<()> {
//     let collectible = Some(get_collectible_data());
//     let issuer_keys = save_mnemonic(
//         &SecretString(ISSUER_MNEMONIC.to_string()),
//         &SecretString("".to_string()),
//     )
//     .await?;
//     let issuer_resp = issuer_issue_contract("RGB21", 1, false, true, collectible).await?;
//     let owner_resp = create_new_invoice(issuer_resp.clone(), None).await?;
//     let psbt_resp = create_new_psbt(issuer_keys.clone(), issuer_resp.clone()).await?;
//     let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

//     let sk = issuer_keys.private.nostr_prv.to_string();
//     let request = SignPsbtRequest {
//         psbt: transfer_resp.psbt,
//         descriptor: SecretString(issuer_keys.private.rgb_udas_descriptor_xprv),
//     };
//     let resp = sign_and_publish_psbt_file(request).await;
//     assert!(resp.is_ok());

//     let request = AcceptRequest {
//         consignment: transfer_resp.consig,
//         force: false,
//     };

//     let resp = accept_transfer(&sk, request).await;
//     assert!(resp.is_ok());
//     assert!(resp?.valid);
//     Ok(())
// }
