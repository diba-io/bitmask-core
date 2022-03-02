use std::str::FromStr;

use anyhow::Result;
use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, FeeRate, Wallet};
use bitcoin::{consensus::serialize, util::address::Address};
use gloo_console::log;

use crate::operations::bitcoin::{balance::synchronize_wallet, sign_psbt::sign_psbt};

pub async fn create_transaction(
    address: String,
    amount: u64,
    wallet: &Wallet<EsploraBlockchain, MemoryDatabase>,
) -> Result<String> {
    synchronize_wallet(wallet).await?;
    let address = Address::from_str(&address[..]);

    let address = match address {
        Ok(address) => address,
        Err(_e) => return Ok("Error on address".to_string()),
    };

    let (psbt, details) = {
        let mut builder = wallet.build_tx();
        builder
            .add_recipient(address.script_pubkey(), amount)
            .enable_rbf()
            .fee_rate(FeeRate::from_sat_per_vb(5.0));
        builder.finish()?
    };

    log!(format!("Transaction details: {:#?}", details));
    log!("Unsigned PSBT: {}", base64::encode(&serialize(&psbt)));
    let signing = sign_psbt(wallet, psbt).await;
    match signing {
        Ok(_signing) => Ok("Ok".to_string()),
        Err(_e) => Ok("Server error".to_string()),
    }
}
