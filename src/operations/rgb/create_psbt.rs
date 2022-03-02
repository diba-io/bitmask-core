use bdk::{blockchain::esplora::EsploraBlockchain, database::MemoryDatabase, FeeRate, Wallet};
use bitcoin::{consensus::serialize, util::address::Address};
use gloo_console::log;

#[allow(dead_code)] // TODO: Is this needed?
pub async fn create_psbt(
    address: Address,
    amount: u64,
    wallet: &Wallet<EsploraBlockchain, MemoryDatabase>,
) {
    let (psbt, details) = {
        let mut builder = wallet.build_tx();
        builder
            .add_recipient(address.script_pubkey(), amount)
            .enable_rbf()
            .fee_rate(FeeRate::from_sat_per_vb(5.0));
        builder.finish().unwrap()
    };

    log!(format!("Transaction details: {:#?}", details));
    log!("Unsigned PSBT: {}", base64::encode(&serialize(&psbt)));
}
