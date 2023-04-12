use std::convert::Infallible;

use amplify::hex::{FromHex, ToHex};
use bitcoin::Transaction;
use bitmask_core::{
    operations::rgb::issue_contract::issue_contract,
    operations::rgb::schemas::default_fungible_iimpl,
};
use bp::{Sats, ScriptPubkey, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray};
use psbt::serialize::Deserialize;
use rgbstd::{
    containers::BindleContent,
    contract::ContractId,
    interface::rgb20,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::ResolveTx as RgbResolveTx,
};
use wallet::onchain::ResolveTx;

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
                txid: bp::Txid::from_hex(&prevout.clone().txid.to_hex()).expect(""),
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
pub fn build_contract(mut stock: Stock) -> (ContractId, Stock) {
    let ticker = "DIBA1";
    let name = "DIBA1";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 1;
    let seal = "tapret1st:c6dd3ff7130c0a15cfdad166ff46fb3f71485f5451e21509027a037298ba1a3b:1";

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

    (contract.contract_id().clone(), stock)
}
