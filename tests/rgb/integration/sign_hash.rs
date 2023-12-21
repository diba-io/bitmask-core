#![cfg(not(target_arch = "wasm32"))]
use amplify::hex::ToHex;
use bdk::wallet::{AddressIndex, AddressInfo};
use bitcoin::{psbt::PartiallySignedTransaction as PsbtV0, EcdsaSighashType};
use bitcoin_blockchain::locks::SeqNo;
use bitmask_core::{
    bitcoin::{
        get_new_address, get_wallet, new_mnemonic, publish_psbt_file, sign_psbt_file, sync_wallet,
    },
    constants::BITCOIN_EXPLORER_API,
    rgb::{
        create_watcher,
        psbt::{NewPsbtOptions, PsbtEx},
        resolvers::ExplorerResolver,
        swap::PsbtSwapEx,
    },
    structs::{
        PrivateWalletData, PublicWalletData, PublishPsbtRequest, SecretString, SignPsbtRequest,
        SignedPsbtResponse, WatcherRequest,
    },
};
use std::str::FromStr;

use miniscript_crate::Descriptor;
use psbt::{serialize::Serialize, Psbt};
use wallet::{
    descriptors::InputDescriptor,
    hd::{DerivationAccount, DerivationSubpath, UnhardenedIndex},
};

use crate::rgb::integration::utils::send_some_coins;

#[tokio::test]
async fn create_auction_signatures() -> anyhow::Result<()> {
    // 1. Initial Setup
    let bob_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let alice_keys = new_mnemonic(&SecretString("".to_string())).await?;
    let charlie_keys = new_mnemonic(&SecretString("".to_string())).await?;

    for participant in [alice_keys.clone()] {
        let watcher_name = "default";
        let participant_pubkey = participant.public.btc_descriptor_xpub.clone();
        let participant_sk = participant.private.nostr_prv.clone();
        let create_watch_req = WatcherRequest {
            name: watcher_name.to_string(),
            xpub: participant.public.watcher_xpub.clone(),
            force: true,
        };
        create_watcher(&participant_sk, create_watch_req.clone()).await?;

        let participant_address = get_new_address(&SecretString(participant_pubkey), None).await?;
        let default_coins = "0.00001000";

        send_some_coins(&participant_address, default_coins).await;
    }

    for participant in [bob_keys.clone(), charlie_keys.clone()] {
        let watcher_name = "default";
        let participant_pubkey = participant.public.btc_descriptor_xpub.clone();
        let participant_sk = participant.private.nostr_prv.clone();
        let create_watch_req = WatcherRequest {
            name: watcher_name.to_string(),
            xpub: participant.public.watcher_xpub.clone(),
            force: true,
        };
        create_watcher(&participant_sk, create_watch_req.clone()).await?;

        let participant_address = get_new_address(&SecretString(participant_pubkey), None).await?;
        let default_coins = "0.01";

        send_some_coins(&participant_address, default_coins).await;
    }

    // 2. Alice Build PSBT Offers (aka. Seller)
    let tx_resolver = ExplorerResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    let PublicWalletData {
        btc_descriptor_xpub: alice_pubkey,
        ..
    } = &alice_keys.public;
    let PrivateWalletData {
        btc_descriptor_xprv: alice_prv,
        ..
    } = &alice_keys.private;
    let alice_descriptor = alice_pubkey.replace("/0/*", "/*/*");
    let alice_descriptor: &Descriptor<DerivationAccount> =
        &Descriptor::from_str(&alice_descriptor)?;

    let alice_prv = SecretString(alice_prv.to_owned());
    let alice_wallet = get_wallet(&alice_prv, None).await?;
    sync_wallet(&alice_wallet).await?;

    let alice_terminal = "/0/0".parse::<DerivationSubpath<UnhardenedIndex>>()?;
    let alice_utxos = alice_wallet.lock().await.list_unspent()?;

    let inputs: Vec<InputDescriptor> = alice_utxos
        .clone()
        .into_iter()
        .map(|x| InputDescriptor {
            outpoint: x.outpoint,
            terminal: alice_terminal.clone(),
            seq_no: SeqNo::default(),
            tweak: None,
            sighash_type: EcdsaSighashType::NonePlusAnyoneCanPay,
        })
        .collect();

    let alice_wallet = get_wallet(&SecretString(alice_pubkey.to_owned()), None).await?;
    sync_wallet(&alice_wallet).await?;

    let AddressInfo {
        address: address_1, ..
    } = alice_wallet.lock().await.get_address(AddressIndex::New)?;

    let bitcoin_fee = 0;
    let change_index = "/1/0".parse::<DerivationSubpath<UnhardenedIndex>>()?;
    let options = NewPsbtOptions::set_inflaction(1_000_u64);

    let alice_outputs = vec![(address_1.script_pubkey().into(), 1_000_u64)];
    let offer_1st = Psbt::new(
        alice_descriptor,
        &inputs,
        &vec![],
        change_index.to_vec(),
        bitcoin_fee,
        &tx_resolver,
        options.clone(),
    )
    .expect("invalid 1st offer psbt");

    let offer_2nd = Psbt::new(
        alice_descriptor,
        &inputs,
        &vec![],
        change_index.to_vec(),
        bitcoin_fee,
        &tx_resolver,
        options.clone(),
    )
    .expect("invalid 1st offer psbt");

    // 3. Bob Build One PSBT Bid (aka. Buyer)
    let PublicWalletData {
        btc_descriptor_xpub: bob_pubkey,
        ..
    } = &bob_keys.public;
    let PrivateWalletData {
        btc_descriptor_xprv: bob_prv,
        ..
    } = &bob_keys.private;
    let bob_descriptor = bob_pubkey.replace("/0/*", "/*/*");
    let bob_descriptor: &Descriptor<DerivationAccount> = &Descriptor::from_str(&bob_descriptor)?;

    let bob_prv = SecretString(bob_prv.to_owned());
    let bob_wallet = get_wallet(&bob_prv, None).await?;
    sync_wallet(&bob_wallet).await?;

    let bob_terminal = "/0/0".parse::<DerivationSubpath<UnhardenedIndex>>()?;
    let bob_utxos = bob_wallet.lock().await.list_unspent()?;

    let inputs: Vec<InputDescriptor> = bob_utxos
        .into_iter()
        .map(|x| InputDescriptor {
            outpoint: x.outpoint,
            terminal: bob_terminal.clone(),
            seq_no: SeqNo::default(),
            tweak: None,
            sighash_type: EcdsaSighashType::NonePlusAnyoneCanPay,
        })
        .collect();

    let change_index = "/1/0".parse::<DerivationSubpath<UnhardenedIndex>>()?;
    let bitcoin_fee = 1_000;

    let options = NewPsbtOptions::default();

    let bid_1st = Psbt::new(
        bob_descriptor,
        &inputs,
        &alice_outputs,
        change_index.to_vec(),
        bitcoin_fee,
        &tx_resolver,
        options,
    )
    .expect("invalid 1st bid psbt");

    // 4. Charlie Build One PSBT Bid (aka. Buyer)
    let PublicWalletData {
        btc_descriptor_xpub: charlie_pubkey,
        ..
    } = &charlie_keys.public;
    let PrivateWalletData {
        btc_descriptor_xprv: charlie_prv,
        ..
    } = &charlie_keys.private;
    let charlie_descriptor = charlie_pubkey.replace("/0/*", "/*/*");
    let charlie_descriptor: &Descriptor<DerivationAccount> =
        &Descriptor::from_str(&charlie_descriptor)?;

    let charlie_prv = SecretString(charlie_prv.to_owned());
    let charlie_wallet = get_wallet(&charlie_prv, None).await?;
    sync_wallet(&charlie_wallet).await?;

    let charlie_terminal = "/0/0".parse::<DerivationSubpath<UnhardenedIndex>>()?;
    let charlie_utxos = charlie_wallet.lock().await.list_unspent()?;

    let inputs: Vec<InputDescriptor> = charlie_utxos
        .into_iter()
        .map(|x| InputDescriptor {
            outpoint: x.outpoint,
            terminal: charlie_terminal.clone(),
            seq_no: SeqNo::default(),
            tweak: None,
            sighash_type: EcdsaSighashType::NonePlusAnyoneCanPay,
        })
        .collect();

    let change_index = "/1/0".parse::<DerivationSubpath<UnhardenedIndex>>()?;
    let bitcoin_fee = 1_000;

    let options = NewPsbtOptions::default();

    let bid_2nd = Psbt::new(
        charlie_descriptor,
        &inputs,
        &alice_outputs,
        change_index.to_vec(),
        bitcoin_fee,
        &tx_resolver,
        options,
    )
    .expect("invalid 1st bid psbt");

    // 5. Create First Swap (Sign)
    let sign_req = SignPsbtRequest {
        psbt: offer_1st.to_string(),
        descriptors: vec![alice_prv.clone()],
    };
    let SignedPsbtResponse { psbt, .. } = sign_psbt_file(sign_req).await?;
    let offer_1st = Psbt::from_str(&psbt)?;

    let sign_req = SignPsbtRequest {
        psbt: bid_1st.to_string(),
        descriptors: vec![bob_prv.clone()],
    };
    let SignedPsbtResponse { psbt, .. } = sign_psbt_file(sign_req).await?;
    let bid_1st = Psbt::from_str(&psbt)?;

    let offer_1st_v0 = PsbtV0::from(offer_1st);
    let bid_1st_v0 = PsbtV0::from(bid_1st);
    let swap_1st = offer_1st_v0.join(bid_1st_v0)?;
    // let swap_1st = Psbt::from(swap_1st);

    // let swap_1st = Serialize::serialize(&swap_1st.clone()).to_hex();
    // let publish_req = PublishPsbtRequest {
    //     psbt: swap_1st.to_string(),
    // };

    // let publish_resp = publish_psbt_file(publish_req).await;
    // assert!(publish_resp.is_ok());

    // 6. Create Second Swap (Sign)
    let sign_req = SignPsbtRequest {
        psbt: offer_2nd.to_string(),
        descriptors: vec![alice_prv.clone()],
    };
    let SignedPsbtResponse { psbt, .. } = sign_psbt_file(sign_req).await?;
    let offer_2nd = Psbt::from_str(&psbt)?;

    let sign_req = SignPsbtRequest {
        psbt: bid_2nd.to_string(),
        descriptors: vec![charlie_prv.clone()],
    };
    let SignedPsbtResponse { psbt, .. } = sign_psbt_file(sign_req).await?;
    let bid_2nd = Psbt::from_str(&psbt)?;

    let offer_2nd_v0 = PsbtV0::from(offer_2nd);
    let bid_2nd_v0 = PsbtV0::from(bid_2nd);
    let swap_2nd = offer_2nd_v0.join(bid_2nd_v0)?;
    // let swap_2nd = Psbt::from(swap_2nd);

    // let swap_2nd = Serialize::serialize(&swap_2nd.clone()).to_hex();
    // let publish_req = PublishPsbtRequest {
    //     psbt: swap_2nd.to_string(),
    // };

    // let publish_resp = publish_psbt_file(publish_req).await;
    // assert!(publish_resp.is_ok());

    // 7. Create Final Swap (Publish)
    let swap_final = swap_1st.join(swap_2nd)?;
    let swap_final = Psbt::from(swap_final);

    let swap_final = Serialize::serialize(&swap_final.clone()).to_hex();
    let publish_req = PublishPsbtRequest {
        psbt: swap_final.to_string(),
    };

    let publish_resp = publish_psbt_file(publish_req).await;
    assert!(publish_resp.is_ok());

    Ok(())
}
