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
        structs::{OutPoint, SealCoins, ThinAsset, TransferRequest, TransferResponse},
    },
    operations::bitcoin::{sign_psbt, synchronize_wallet},
};

pub async fn transfer_asset(
    blinded_utxo: String,
    amount: u64,
    asset: ThinAsset,
    wallet: &Wallet<MemoryDatabase>,
) -> Result<String> {
    synchronize_wallet(wallet).await?;
    log!("sync");
    let unspents = wallet.list_unspent()?;
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
    let seal_coins: Vec<SealCoins> = unspents
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
        .map(|x| SealCoins {
            coins: asset.balance.unwrap() - amount,
            txid: Some(x.outpoint.txid.to_string()),
            vout: x.outpoint.vout,
        })
        .collect();
    log!("seal_coins");
    log!(format!("{:#?}", &seal_coins));

    let seal_coins = if seal_coins.is_empty() {
        vec![seal_coins[0].clone()]
    } else {
        vec![]
    };

    let send_to = wallet.get_address(New).unwrap();
    let (psbt, _details) = {
        let mut builder = wallet.build_tx();
        builder
            .add_recipient(send_to.script_pubkey(), 1000)
            .enable_rbf()
            .fee_rate(FeeRate::from_sat_per_vb(1.0));
        builder.finish()?
    };

    log!("psbt");
    log!(psbt.to_string());
    log!("to the server");
    let transfer_request = TransferRequest {
        inputs: utxos,
        allocate: seal_coins,
        receiver: blinded_utxo.to_string(),
        amount,
        asset: asset.id,
        witness: base64::encode(&serialize(&psbt)),
    };
    log!(format!("{:?}", transfer_request));

    let url = format!("{}transfer", *NODE_SERVER_BASE_URL);
    let response = Request::post(&url)
        .body(serde_json::to_string(&transfer_request)?)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    log!("made");
    // parse into generic JSON value
    let js: TransferResponse = response.json().await?;
    log!("deserialized");
    let psbt: PartiallySignedTransaction = deserialize(&base64::decode(js.witness.clone())?)?;
    sign_psbt(wallet, psbt).await?;
    log!(format!("Transfer made: {js:?}"));
    Ok(js.consignment)
}
