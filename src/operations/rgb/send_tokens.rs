use anyhow::Result;
use bdk::{database::MemoryDatabase, wallet::AddressIndex::New, FeeRate, Wallet};
use bitcoin::{
    consensus::{deserialize, serialize},
    util::psbt::PartiallySignedTransaction,
};
use gloo_console::log;
use gloo_net::http::Request;

use crate::{
    data::{
        constants::NODE_SERVER_BASE_URL,
        structs::{
            EncloseForgetRequest, OutPoint, SealCoins, ThinAsset, TransferRequest, TransferResponse,
        },
    },
    operations::bitcoin::{sign_psbt, synchronize_wallet},
};

pub async fn transfer_asset(
    blinded_utxo: String,
    amount: u64,
    asset: ThinAsset,
    full_wallet: &Wallet<MemoryDatabase>,
    full_change_wallet: &Wallet<MemoryDatabase>,
    assets_wallet: &Wallet<MemoryDatabase>,
    node_url: Option<String>,
) -> Result<String> {
    synchronize_wallet(assets_wallet).await?;
    log!("sync");
    let unspents = assets_wallet.list_unspent()?;
    let utxos: Vec<OutPoint> = asset
        .allocations
        .clone()
        .into_iter()
        .map(|x| {
            let mut split = x.outpoint.split(':');
            OutPoint {
                txid: split.next().unwrap().to_string(),
                vout: split.next().unwrap().to_string().parse::<u32>().unwrap(),
            }
        })
        .collect();
    log!(format!("utxos {utxos:#?}"));
    log!(format!("unspents {unspents:#?}"));

    let to_be_consumed_utxos = utxos.clone();
    let unspendable_outputs: Vec<bitcoin::OutPoint> = unspents
        .into_iter()
        .filter(move |x| {
            let mut pass = true;
            for local_utxo in to_be_consumed_utxos.iter() {
                log!(format!("local_utxo {local_utxo:#?}"));
                if (local_utxo.txid == x.outpoint.txid.to_string())
                    && (local_utxo.vout == x.outpoint.vout)
                {
                    log!(format!("local outpoint {:#?}", &x.outpoint));
                    pass = false;
                    break;
                }
            }
            pass
        })
        .map(|x| bitcoin::OutPoint {
            txid: x.outpoint.txid,
            vout: x.outpoint.vout,
        })
        .collect();

    let seal_coins: Vec<SealCoins> = unspendable_outputs
        .clone()
        .into_iter()
        .map(|x| SealCoins {
            coins: asset.balance.unwrap() - amount,
            txid: Some(x.txid.to_string()),
            vout: x.vout,
        })
        .filter(|x| (x.coins > 0)) // TODO: if we have only one asset it's all well but if we have several it will fail. Problem is we need allocate if we have serveral but if you put it 0 it will fail, so maybe is a rgb-node problem
        .collect();
    log!("seal_coins");
    log!(format!("{:#?}", &seal_coins));

    let seal_coins = match seal_coins.get(0) {
        Some(seal_coin) => {
            vec![seal_coin.clone()]
        }
        None => {
            vec![]
        }
    };

    let send_to_one = assets_wallet.get_address(New).unwrap(); // TODO: that has to be corrected before release because this sats are lost! Bdk don't get the tweak key utxos!
    let send_to_two = assets_wallet.get_address(New).unwrap();
    synchronize_wallet(full_wallet).await?;
    let (psbt, _details) = {
        let mut builder = full_wallet.build_tx();
        builder
            .unspendable(unspendable_outputs.clone())
            .add_recipient(send_to_one.script_pubkey(), 546)
            .add_recipient(send_to_two.script_pubkey(), 546)
            .enable_rbf()
            .fee_rate(FeeRate::from_sat_per_vb(1.0));
        match builder.finish() {
            Ok((psbt, details)) => (psbt, details),
            Err(e) => {
                log!(format!("{:#?}", e));
                builder = full_change_wallet.build_tx();
                builder
                    .unspendable(unspendable_outputs)
                    .add_recipient(send_to_one.script_pubkey(), 546)
                    .add_recipient(send_to_two.script_pubkey(), 546)
                    .fee_rate(FeeRate::from_sat_per_vb(1.0));
                builder.finish()?
            }
        }
    };

    log!(format!("psbt to server {:#?}", psbt.to_string()));
    let transfer_request = TransferRequest {
        inputs: utxos.clone(),
        allocate: seal_coins,
        receiver: blinded_utxo,
        amount,
        asset: asset.id,
        witness: base64::encode(&serialize(&psbt)),
    };
    log!(format!("{:?}", transfer_request));

    let node_url = node_url.unwrap_or(NODE_SERVER_BASE_URL.to_string());
    let url = format!("{}transfer", node_url);
    let response = Request::post(&url)
        .body(serde_json::to_string(&transfer_request)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await
        .unwrap_or_else(|e| {
            log!(format!("error from server {:#?}", e));
            panic!("{:#?}", e);
        });
    // parse into generic JSON value
    let js: TransferResponse = response.json().await?;
    let psbt: PartiallySignedTransaction = deserialize(&base64::decode(js.witness.clone())?)?;
    log!(format!("psbt from server {:#?}", psbt.to_string()));
    sign_psbt(full_wallet, psbt).await.unwrap_or_else(|e| {
        log!(format!("error at signing: {:#?}", e));
    });

    let url = format!("{}enclose_forget", node_url);
    let enclose_request = EncloseForgetRequest {
        outpoints: utxos,
        disclosure: js.disclosure.clone(),
    };
    let response = Request::post(&url)
        .body(serde_json::to_string(&enclose_request)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await?;

    let status = response.status();
    log!(format!("enclose and forget made {:?}", status));

    if status == 200 {
        let response = response.text().await?;
        log!(format!("forget utxo success: {:#?}", response));
    } else {
        log!(format!("forget utxo error"));
    }

    log!(format!("Transfer made: {js:?}"));
    Ok(serde_json::to_string(&js).unwrap())
}
