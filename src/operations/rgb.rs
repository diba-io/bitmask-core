mod accept_transaction;
mod create_psbt;
mod descriptor_wallet;
mod import_asset;
mod init;
mod issue_asset;
mod receive_tokens;
mod rpc;
mod send_tokens;
mod validate_transaction;

pub use accept_transaction::accept_transfer;
pub use create_psbt::create_psbt;
pub use import_asset::{get_asset_by_genesis, get_assets};
pub use init::{inproc, rgb_init};
pub use issue_asset::issue_asset;
pub use receive_tokens::blind_utxo;
pub use rpc::{register_contract, rgb_cli, transfer_compose};
pub use send_tokens::transfer_asset;
pub use validate_transaction::validate_transfer;

// pub fn rgb20_transfer(
//     assets_wallet: &Wallet<AnyDatabase>,
//     contract_str: &str,
//     amount: u64,
//     blinded_utxo: &str,
//     bdk_rgb_assets_descriptor_xpub: &str,
// ) -> Result<(
//     Asset,
//     Vec<AssignedState<Revealed>>,
//     Vec<LocalUtxo>,
//     Vec<OutPoint>,
// )> {
//     let contract = Contract::from_str(contract_str)?;
//     debug!(format!("parsed contract: {contract}"));
//     let asset = Asset::try_from(&contract)?;
//     debug!(format!("asset from contract: {asset:#?}"));

//     let asset_utxos = assets_wallet.list_unspent()?;

//     let mut allocations = vec![];
//     let mut balance = 0;

//     for utxo in &asset_utxos {
//         let mut coins = asset.outpoint_coins(utxo.outpoint);
//         for coin in coins.iter() {
//             balance += coin.state.value;
//         }
//         allocations.append(&mut coins);
//     }

//     trace!(format!("asset utxos {:#?}", &asset_utxos));
//     debug!(format!("allocations {allocations:#?}"));
//     debug!(format!("balance {balance}"));

//     if amount > balance {
//         error!(format!(
//             "Not enough coins. Had {balance}, but needed {amount}"
//         ));
//         return Err(anyhow!(
//             "Not enough coins. Had {balance}, but needed {amount}"
//         ));
//     }

//     let seal_coins: Vec<SealCoins> = allocations
//         .clone()
//         .into_iter()
//         .map(|coin| SealCoins {
//             amount: coin.state.value,
//             txid: coin.seal.txid,
//             vout: coin.seal.vout,
//         })
//         // TODO: If we have only one asset it's okay, but if we have several it will fail. We need to allocate if we have several but if you put in 0 it will fail, so it might be an rgb-node problem
//         .filter(|x| (x.amount > 0))
//         .collect();
//     info!("seal_coins", format!("{seal_coins:#?}"));

//     let outpoints: Vec<OutPoint> = seal_coins
//         .iter()
//         .map(|coin| OutPoint {
//             txid: coin.txid,
//             vout: coin.vout,
//         })
//         .collect();

//     let transfer_consignment = transfer_compose(vec![], contract.contract_id(), outpoints)?;

//     //     Ok((asset, allocations, asset_utxos, outpoints))
//     // }

//     // pub fn rgb20_transfer(
//     //     asset: &Asset,
//     //     allocations: Vec<AssignedState<Revealed>>,
//     //     asset_utxos: Vec<LocalUtxo>,
//     //     amount: u64,
//     //     blinded_utxo: &str,
//     // ) -> Result<()> {
//     info!(format!("Parse blinded UTXO: {blinded_utxo}"));
//     let utxob = match blinded_utxo.parse() {
//         Ok(utxob) => utxob,
//         Err(err) => return Err(anyhow!("Error parsing supplied blinded utxo: {err}")),
//     };

//     // rust-rgb20 -> bin/rgb20 -> Command::Transfer
//     let beneficiaries = vec![UtxobValue {
//         value: amount,
//         seal_confidential: utxob,
//     }];
//     debug!("Map beneficiaries");
//     let beneficiaries = beneficiaries
//         .into_iter()
//         .map(|v| (v.seal_confidential.into(), amount))
//         .collect();

//     info!(format!("Beneficiaries: {beneficiaries:#?}"));

//     debug!("Coin selection - Largest First Coin");
//     let mut change: Vec<(AssignedState<_>, u64)> = vec![];
//     let mut inputs = vec![];
//     let mut remainder = amount;

//     for coin in allocations {
//         let descriptor = format!("{}:{} /0/0", coin.seal.txid, coin.seal.vout);
//         debug!(format!(
//             "Parsing InputDescriptor from outpoint: {descriptor}"
//         ));
//         let input_descriptor = match InputDescriptor::from_str(&descriptor) {
//             Ok(desc) => desc,
//             Err(err) => return Err(anyhow!("Error parsing input_descriptor: {err}")),
//         };
//         debug!(format!(
//             "InputDescriptor successfully parsed: {input_descriptor:#?}"
//         ));

//         if coin.state.value >= remainder {
//             debug!("Large coins");
//             // TODO: Change output must not be cloned, it needs to be a separate UTXO
//             change.push((coin.clone(), coin.state.value - remainder)); // Change
//             inputs.push(input_descriptor);
//             debug!(format!("Coin: {coin:#?}"));
//             debug!(format!(
//                 "Amount: {} - Remainder: {remainder}",
//                 coin.state.value
//             ));
//             break;
//         } else {
//             debug!("Whole coins");
//             change.push((coin.clone(), coin.state.value)); // Spend entire coin
//             remainder -= coin.state.value;
//             inputs.push(input_descriptor);
//             debug!(format!("Coin: {coin:#?}"));
//             debug!(format!(
//                 "Amount: {} - Remainder: {remainder}",
//                 coin.state.value
//             ));
//         }
//     }

//     debug!(format!("Change: {change:#?}"));
//     debug!(format!("Inputs: {inputs:#?}"));

//     // Find an output that isn't being used as change
//     let change_outputs: Vec<&LocalUtxo> = asset_utxos
//         .iter()
//         .filter(|asset_utxo| {
//             !change.iter().any(|(coin, _)| {
//                 coin.seal.txid == asset_utxo.outpoint.txid
//                     && coin.seal.vout == asset_utxo.outpoint.vout
//             })
//         })
//         .collect();

//     trace!(format!("Candidate change outputs: {change_outputs:#?}"));

//     // If there's no free outputs, the user needs to run fund vault again.
//     if change_outputs.is_empty() {
//         error!("no free outputs, the user needs to run fund vault again");
//         return Err(anyhow!(
//             "no free outputs, the user needs to run fund vault again"
//         ));
//     }
//     let change_output = change_outputs.get(0).unwrap();
//     debug!(format!("Selected change output: {change_output:#?}"));

//     let change = change
//         .iter()
//         .map(|(_coin, remainder)| AllocatedValue {
//             value: *remainder,
//             seal: ExplicitSeal {
//                 method: CloseMethod::TapretFirst,
//                 txid: Some(change_output.outpoint.txid),
//                 vout: change_output.outpoint.vout,
//             },
//         })
//         .map(|v| (v.into_revealed_seal(), v.value))
//         .collect();

//     let outpoints: BTreeSet<OutPoint> = outpoints.into_iter().collect();

//     info!("Creating state transition for asset transfer");
//     debug!(format!("Outpoints: {outpoints:#?}"));
//     debug!(format!("Beneficiaries: {beneficiaries:#?}"));
//     debug!(format!("Change allocated values: {change:#?}"));

//     let transition = match asset.transfer(outpoints.clone(), beneficiaries, change) {
//         Ok(t) => t,
//         Err(err) => {
//             error!(format!(
//                 "Error creating state transition for asset transfer: {err}",
//             ));
//             return Err(anyhow!(
//                 "Error creating state transition for asset transfer"
//             ));
//         }
//     };

//     info!("Successfully created transition");
//     debug!(format!("Transition: {transition:#?}"));

//     // descriptor-wallet -> btc-cold -> construct
//     // btc-cold construct --input "${UTXO_SRC} /0/0" --allow-tapret-path 1 ${WALLET} ${PSBT} ${FEE}
//     let url = BITCOIN_ELECTRUM_API.read().unwrap();
//     let electrum_client = ElectrumClient::new(&url)?;
//     debug!(format!("Electrum client connected to {url}"));

//     let txid_set: BTreeSet<_> = inputs.iter().map(|input| input.outpoint.txid).collect();
//     debug!(format!("txid set: {txid_set:?}"));

//     let tx_map = electrum_client
//         .batch_transaction_get(&txid_set)?
//         .into_iter()
//         .map(|tx| (tx.txid(), tx))
//         .collect::<BTreeMap<_, _>>();

//     info!("Re-scanned network");

//     let outputs = vec![]; // TODO: not sure if this is correct
//     let allow_tapret_path = DfsPath::from_str("1")?;

//     // format BDK descriptor for RGB
//     let re = Regex::new(r"\(\[([0-9a-f]+)/(.+)](.+?)/").unwrap();
//     let cap = re.captures(bdk_rgb_assets_descriptor_xpub).unwrap();
//     let rgb_assets_descriptor = format!("tr(m=[{}]/{}=[{}]/*/*)", &cap[1], &cap[2], &cap[3]);
//     let rgb_assets_descriptor = rgb_assets_descriptor.replace('\'', "h");

//     debug!(format!(
//         "Creating descriptor wallet from RGB Tokens Descriptor: {rgb_assets_descriptor}"
//     ));
//     let descriptor = match Descriptor::from_str(&rgb_assets_descriptor) {
//         Ok(d) => d,
//         Err(err) => {
//             error!(format!(
//                 "Error creating descriptor wallet from RGB Tokens Descriptor: {err}",
//             ));
//             return Err(anyhow!(
//                 "Error creating descriptor wallet from RGB Tokens Descriptor"
//             ));
//         }
//     };
//     let fee = 500;

//     debug!("Constructing PSBT with...");
//     debug!(format!("outputs: {outputs:?}"));
//     debug!(format!("allow_tapret_path: {allow_tapret_path:?}"));
//     debug!(format!("descriptor: {descriptor:#?}"));
//     debug!(format!("fee: {fee:?}"));

//     Ok(())
// }
