use std::{
    collections::{BTreeMap, HashMap},
    convert::Infallible,
    str::FromStr,
};

use amplify::hex::{FromHex, ToHex};
use bitcoin::Transaction;
use bitmask_core::{
    rgb::issue::issue_contract,
    rgb::transfer::create_invoice,
    structs::{IssueMetaRequest, IssueMetadata, MediaInfo},
};
use bp::{
    LockTime, Outpoint, Sats, ScriptPubkey, SeqNo, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray,
    Witness,
};
use psbt::{serialize::Deserialize, Psbt};
use rgbstd::{
    containers::BindleContent,
    contract::{ContractId, WitnessOrd},
    persistence::Stock,
    resolvers::ResolveHeight,
    validation::ResolveTx as RgbResolveTx,
};
use rgbwallet::RgbInvoice;
use wallet::onchain::ResolveTx;

// Resolvers
pub struct DumbResolve {}

impl ResolveHeight for DumbResolve {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> std::result::Result<WitnessOrd, Self::Error> {
        Ok(WitnessOrd::OffChain)
    }
}
impl ResolveTx for DumbResolve {
    fn resolve_tx(
        &self,
        _txid: bitcoin::Txid,
    ) -> Result<Transaction, wallet::onchain::TxResolverError> {
        let hex = "020000000001019d8420cc5666b02f260bbaea43326c50a2c2eb99292fcf4c42a6179e132344de0000000000fdffffff02db9a8b44000000002251205d853a4a3da1dc163d2a2d9e8a76ae63db83f9310a25caa5d216a0fd962923a900e1f505000000002251206a61bf8aea7388b8541f16d773b77f897110eaa6bc17ada61c50bc70a93e5d610247304402202814bbcab5708f17d3e8ad42100ea1c156bbce287260d3394587339142767451022079d1c3bbe495fa57a0fab035c09a255502264d8bc3249f3ac5cd4c8878b91e0e012102c7c433670742289165c540c733d3473a7f458126e2a85c1b86b6b975a4ef5739f4010000";
        let transaction = Transaction::deserialize(&Vec::from_hex(hex).unwrap()).unwrap();
        Ok(transaction)
    }
}

impl RgbResolveTx for DumbResolve {
    fn resolve_tx(&self, _txid: Txid) -> Result<bp::Tx, rgbstd::validation::TxResolverError> {
        let hex = "020000000001014fba153e23558ca5532b5187ac20c4e35fe588c9bcb4a7b3c881c0541fcda65c0100000000ffffffff0118ddf50500000000225120d9b9957aa15bb91d856ed862cd04183555c9b9ea04ec3763c3b1e388adebe8e601417b5df1ce9c9c56c914203d8b2827000c72a15733e85f18c6a35f1fafa9c5068a8c73169dc3d98113112d7309114ca449fe3f740e949dbc6712ff945115d666c10100000000";
        let transaction = Transaction::deserialize(&Vec::from_hex(hex).unwrap()).unwrap();

        let mut ti = VarIntArray::new();
        let tx_input = &transaction.input[0];
        let input = TxIn {
            prev_output: Outpoint::new(
                Txid::from_str(&tx_input.previous_output.txid.to_hex()).expect("oh no!"),
                tx_input.previous_output.vout,
            ),
            sig_script: tx_input.script_sig.to_bytes().into(),
            sequence: SeqNo::from_consensus_u32(tx_input.sequence.to_consensus_u32()),
            witness: Witness::from_consensus_stack(tx_input.witness.to_vec()),
        };
        ti.push(input).expect("fail");

        let mut to = VarIntArray::new();
        let tx_output = &transaction.output[0];
        let output = TxOut {
            value: Sats::from(tx_output.value),
            script_pubkey: ScriptPubkey::from(tx_output.script_pubkey.to_bytes()),
        };
        to.push(output).expect("fail");

        let tx = Tx {
            version: TxVer::V2,
            inputs: ti,
            outputs: to,
            lock_time: LockTime::from_consensus_u32(422),
        };
        Ok(tx)
    }
}

// Helpers
#[allow(dead_code)]
pub fn create_fake_psbt() -> Psbt {
    let psbt_hex = "70736274ff01005e02000000014fba153e23558ca5532b5187ac20c4e35fe588c9bcb4a7b3c881c0541fcda65c0100000000ffffffff0118ddf505000000002251202aa594ee4dc05d289387c77a44ee3d5401a7edc269e355f2345c2792d9f8d014000000004f01043587cf034a3acf0b80000000fe80c9c11d65f2a2bfbf8e582c49b829e0f453e2a7138ec303ddd724aa295ebf02008b0bc2899bf59a892479c553d7c7e6901a0fc8db3e5570529101bc783743bf10280a59635600008001000080000000800001008902000000019d8420cc5666b02f260bbaea43326c50a2c2eb99292fcf4c42a6179e132344de0000000000fdffffff02db9a8b44000000002251205d853a4a3da1dc163d2a2d9e8a76ae63db83f9310a25caa5d216a0fd962923a900e1f505000000002251206a61bf8aea7388b8541f16d773b77f897110eaa6bc17ada61c50bc70a93e5d61f4010000010304010000002116e7e50584e394cb1b467f440e8760bf3806835d55378f78cbacb8c651d2e11d0f1900280a59635600008001000080000000800000000000000000011720e7e50584e394cb1b467f440e8760bf3806835d55378f78cbacb8c651d2e11d0f0022020269c3a787c625331a17fd8a5cf7094d4672fb0385b5fd8fa2813181de3a1cef3e18280a5963560000800100008000000080010000000000000001052069c3a787c625331a17fd8a5cf7094d4672fb0385b5fd8fa2813181de3a1cef3e09fc06544150524554000000";
    Psbt::from_str(psbt_hex).expect("invalid dumb psbt")
}

#[allow(dead_code)]
pub fn create_fake_contract(stock: &mut Stock) -> ContractId {
    let ticker = "DIBA";
    let name = "DIBA";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 10;
    let seal = "tapret1st:5ca6cd1f54c081c8b3a7b4bcc988e55fe3c420ac87512b53a58c55233e15ba4f:1";
    let network = "regtest";
    let iface = "RGB20";
    let mut resolver = DumbResolve {};

    let contract = issue_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        iface,
        seal,
        network,
        None,
        BTreeMap::new(),
        &mut resolver,
        stock,
    )
    .expect("test issue_contract failed");

    let mut dumb = DumbResolve {};

    let bindle = contract.bindle();
    let contract = bindle
        .unbindle()
        .validate(&mut dumb)
        .map_err(|c| c.validation_status().expect("just validated").to_string())
        .expect("invalid contract");

    contract.contract_id()
}

#[allow(dead_code)]
pub fn create_fake_invoice(contract_id: ContractId, seal: &str, stock: &mut Stock) -> RgbInvoice {
    let amount = 1;
    let iface = "RGB20";
    let params = HashMap::new();
    create_invoice(
        &contract_id.to_string(),
        iface,
        amount,
        seal,
        "regtest",
        params,
        stock,
    )
    .expect("create_invoice failed")
}

#[allow(dead_code)]
pub fn get_uda_data() -> IssueMetaRequest {
    IssueMetaRequest::with(IssueMetadata::UDA(vec![MediaInfo {
        ty: "image/png".to_string(),
        source: "https://carbonado.io/diba.png".to_string(),
    }]))
}
