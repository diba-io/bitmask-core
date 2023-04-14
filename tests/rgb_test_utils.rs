use std::{convert::Infallible, str::FromStr};

use amplify::hex::{FromHex, ToHex};
use bitcoin::Transaction;
use bitmask_core::{
    operations::rgb::issue::issue_contract,
    operations::rgb::{invoice::create_invoice, schemas::default_fungible_iimpl},
};
use bp::{Sats, ScriptPubkey, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray};
use psbt::{serialize::Deserialize, Psbt};
use rgbstd::{
    containers::BindleContent,
    contract::{ContractId, GraphSeal},
    interface::rgb20,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::ResolveTx as RgbResolveTx,
};
use rgbwallet::RgbInvoice;
use wallet::onchain::ResolveTx;

// Constants

// Resolvers
pub struct DumbResolve {}

impl ResolveHeight for DumbResolve {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> std::result::Result<u32, Self::Error> {
        Ok(0)
    }
}
impl ResolveTx for DumbResolve {
    fn resolve_tx(
        &self,
        _txid: bitcoin::Txid,
    ) -> Result<Transaction, wallet::onchain::TxResolverError> {
        let hex = "02000000000101e69244b22f5cffce43ef4fdb8ade5af7c1e14eeb65253a125bf94ecab1f789960000000000fdffffff02db9a8b4400000000225120be03966f51ab5173f676f9858c7befbd078679bb88fa6d22b56c0e05a998747400e1f505000000002251206a61bf8aea7388b8541f16d773b77f897110eaa6bc17ada61c50bc70a93e5d610247304402202a476920817a392edf51a4b18b12ea80ccc0d541962fbd7533a2354d4dac4992022008e8eab52e29e9e4bc98e924bb4d2586c9cdd3d7ffc8367e77cd57372c6bad7701210209c67298b7958a3e0e81658ce1b665c65e752dd6cab57b63a40b1bc816622c9da6010000";
        let transaction = Transaction::deserialize(&Vec::from_hex(hex).unwrap()).unwrap();
        Ok(transaction)
    }
}

impl RgbResolveTx for DumbResolve {
    fn resolve_tx(&self, _txid: Txid) -> Result<bp::Tx, rgbstd::validation::TxResolverError> {
        let hex = "02000000000101e69244b22f5cffce43ef4fdb8ade5af7c1e14eeb65253a125bf94ecab1f789960000000000fdffffff02db9a8b4400000000225120be03966f51ab5173f676f9858c7befbd078679bb88fa6d22b56c0e05a998747400e1f505000000002251206a61bf8aea7388b8541f16d773b77f897110eaa6bc17ada61c50bc70a93e5d610247304402202a476920817a392edf51a4b18b12ea80ccc0d541962fbd7533a2354d4dac4992022008e8eab52e29e9e4bc98e924bb4d2586c9cdd3d7ffc8367e77cd57372c6bad7701210209c67298b7958a3e0e81658ce1b665c65e752dd6cab57b63a40b1bc816622c9da6010000";
        let transaction = Transaction::deserialize(&Vec::from_hex(hex).unwrap()).unwrap();

        let mut ti = VarIntArray::new();
        let tx_input = &transaction.input[0];
        let prevout = &transaction.input[0].previous_output;
        let input = TxIn {
            prev_output: bp::Outpoint {
                txid: bp::Txid::from_hex(&prevout.txid.to_hex()).expect(""),
                vout: bp::Vout::from(prevout.vout),
            },
            sequence: bp::SeqNo::from(tx_input.sequence.0),
            sig_script: bp::SigScript::default(),
        };
        ti.push(input).expect("");

        let mut to = VarIntArray::new();
        let tx_output = &transaction.output[0];
        let output = TxOut {
            value: Sats::from(tx_output.value),
            script_pubkey: ScriptPubkey::default(),
        };
        to.push(output).expect("");

        let tx_output = &transaction.output[1];
        let output = TxOut {
            value: Sats::from(tx_output.value),
            script_pubkey: ScriptPubkey::default(),
        };
        to.push(output).expect("");

        let tx = Tx {
            version: TxVer::V2,
            inputs: ti,
            outputs: to,
            lock_time: 422.into(),
        };
        Ok(tx)
    }
}

// Helpers
pub fn dumb_contract(mut stock: Stock) -> (ContractId, Stock) {
    let ticker = "DIBA1";
    let name = "DIBA1";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 10;
    let seal = "tapret1st:70339a6b27f55105da2d050babc759f046c21c26b7b75e9394bc1d818e50ff52:0";

    let iface = rgb20();
    let iimpl = default_fungible_iimpl();

    let contract = issue_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        seal,
        iface,
        iimpl,
    )
    .expect("");

    let mut dumb = DumbResolve {};

    let bindle = contract.bindle();
    let contract = bindle
        .unbindle()
        .validate(&mut dumb)
        .map_err(|c| c.validation_status().expect("just validated").to_string())
        .expect("");

    stock
        .import_contract(contract.clone(), &mut dumb)
        .expect("");

    (contract.contract_id(), stock)
}

pub fn dumb_psbt() -> Psbt {
    let psbt_hex = "70736274ff01005e020000000152ff508e811dbc94935eb7b7261cc246f059c7ab0b052dda0551f5276b9a33700000000000ffffffff0118ddf505000000002251202aa594ee4dc05d289387c77a44ee3d5401a7edc269e355f2345c2792d9f8d014000000004f01043587cf034a3acf0b80000000fe80c9c11d65f2a2bfbf8e582c49b829e0f453e2a7138ec303ddd724aa295ebf02008b0bc2899bf59a892479c553d7c7e6901a0fc8db3e5570529101bc783743bf10280a5963560000800100008000000080000100890200000001984806892c61298e3b0117e501bed6aa9cb523a49ad4b9e9c71dd818f8e7b5520000000000fdffffff0200e1f505000000002251206a61bf8aea7388b8541f16d773b77f897110eaa6bc17ada61c50bc70a93e5d61db9a8b4400000000225120c1a028a16aa6a3ebdf30bcd11ad6a0084298a7b097a97624c5b7ad2df5748c30f4010000010304010000002116e7e50584e394cb1b467f440e8760bf3806835d55378f78cbacb8c651d2e11d0f1900280a59635600008001000080000000800000000000000000011720e7e50584e394cb1b467f440e8760bf3806835d55378f78cbacb8c651d2e11d0f0022020269c3a787c625331a17fd8a5cf7094d4672fb0385b5fd8fa2813181de3a1cef3e18280a5963560000800100008000000080010000000000000001052069c3a787c625331a17fd8a5cf7094d4672fb0385b5fd8fa2813181de3a1cef3e09fc06544150524554000000";

    Psbt::from_str(psbt_hex).expect("")
}

pub fn dumb_invoice(contract_id: ContractId, stock: Stock, txid: String, vout: u32) -> RgbInvoice {
    let amount = 1;
    let iface = rgb20();
    let txid: Txid = txid.parse().expect("");
    let seal = GraphSeal::tapret_first(txid, vout);

    create_invoice(contract_id, iface, amount, seal, stock).expect("")
}
