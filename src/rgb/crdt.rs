use amplify::hex::ToHex;
use autosurgeon::{Hydrate, Reconcile};
use bitcoin::OutPoint;
use bitcoin_30::bip32::ExtendedPubKey;
use bp::{dbc::tapret::TapretCommitment, Outpoint};
use rgb::{DeriveInfo, RgbDescr, RgbWallet, Tapret, TerminalPath, Utxo};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use crate::rgb::{
    structs::{RgbAccountV0, RgbAccountV1},
    swap::{RgbAuctionSwaps, RgbBidSwap, RgbPublicSwaps},
};

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbMergeError {
    /// Invalid Tapret Wallet Format
    NoTapret,
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone, Reconcile, Hydrate, Default, Display)]
#[display(doc_comments)]
pub struct RawRgbAccount {
    pub wallets: HashMap<String, RawRgbWallet>,
    pub hidden_contracts: Vec<String>,
    pub invoices: Vec<String>,
}

impl From<RgbAccountV0> for RawRgbAccount {
    fn from(wallet: RgbAccountV0) -> Self {
        Self {
            wallets: wallet
                .wallets
                .into_iter()
                .map(|(name, wallet)| (name, RawRgbWallet::from(wallet)))
                .collect(),
            ..Default::default()
        }
    }
}

impl From<RgbAccountV1> for RawRgbAccount {
    fn from(wallet: RgbAccountV1) -> Self {
        Self {
            hidden_contracts: wallet.hidden_contracts,
            invoices: wallet.invoices,
            wallets: wallet
                .wallets
                .into_iter()
                .map(|(name, wallet)| (name, RawRgbWallet::from(wallet)))
                .collect(),
        }
    }
}

impl From<RawRgbAccount> for RgbAccountV1 {
    fn from(raw_account: RawRgbAccount) -> Self {
        Self {
            hidden_contracts: raw_account.hidden_contracts,
            invoices: raw_account.invoices,
            wallets: raw_account
                .wallets
                .into_iter()
                .map(|(name, wallet)| (name, RgbWallet::from(wallet)))
                .collect(),
        }
    }
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone, Reconcile, Hydrate, Default, Display)]
#[display(doc_comments)]
pub struct RawRgbWallet {
    pub xpub: String,
    pub taprets: BTreeMap<String, Vec<String>>,
    pub utxos: Vec<RawUtxo>,
}

impl From<RgbWallet> for RawRgbWallet {
    fn from(wallet: RgbWallet) -> Self {
        let RgbDescr::Tapret(tapret) = wallet.descr;

        let mut raw_uxtos = vec![];
        let mut raw_taprets = BTreeMap::new();
        for (terminal_path, taprets) in tapret.taprets {
            let TerminalPath { app, index } = terminal_path;
            raw_taprets.insert(
                format!("{app}:{index}"),
                taprets.into_iter().map(|tap| tap.to_string()).collect(),
            );
        }

        for utxo in wallet.utxos {
            raw_uxtos.push(RawUtxo::from(utxo));
        }

        Self {
            xpub: tapret.xpub.to_string(),
            taprets: raw_taprets,
            utxos: raw_uxtos,
        }
    }
}

impl From<RawRgbWallet> for RgbWallet {
    fn from(raw_wallet: RawRgbWallet) -> Self {
        let xpub = ExtendedPubKey::from_str(&raw_wallet.xpub).expect("invalid xpub");
        let mut tapret = Tapret {
            xpub,
            taprets: BTreeMap::new(),
        };

        for (raw_terminal, raw_tweaks) in raw_wallet.taprets {
            let mut split = raw_terminal.split(':');
            let app = split.next().unwrap();
            let index = split.next().unwrap();

            let terminal = TerminalPath {
                app: app.parse().expect("invalid terminal path app"),
                index: index.parse().expect("invalid terminal path index"),
            };

            let taprets = raw_tweaks
                .into_iter()
                .map(|tap| TapretCommitment::from_str(&tap).expect("invalid taptweak format"))
                .collect();

            tapret.taprets.insert(terminal, taprets);
        }
        Self {
            descr: RgbDescr::Tapret(tapret),
            utxos: raw_wallet.utxos.into_iter().map(Utxo::from).collect(),
        }
    }
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone, Reconcile, Hydrate, Default, Display)]
#[display(doc_comments)]
pub struct RawUtxo {
    pub outpoint: String,
    pub block: u32,
    pub amount: u64,
    // DeriveInfo,
    pub terminal: String,
    pub tweak: Option<String>,
}

impl From<RawUtxo> for Utxo {
    fn from(raw_utxo: RawUtxo) -> Self {
        let mut split = raw_utxo.terminal.split(':');
        let app = split.next().unwrap();
        let index = split.next().unwrap();

        let terminal = TerminalPath {
            app: app.parse().expect("invalid terminal path app"),
            index: index.parse().expect("invalid terminal path index"),
        };

        let mut tweak = none!();
        if let Some(taptweak) = raw_utxo.tweak {
            tweak = Some(TapretCommitment::from_str(&taptweak).expect("invalid taptweak format"));
        }

        let derive_info = DeriveInfo { terminal, tweak };

        let outpoint = OutPoint::from_str(&raw_utxo.outpoint).expect("invalid outpoint parse");
        let txid = bp::Txid::from_str(&outpoint.txid.to_hex()).expect("invalid txid");

        Self {
            outpoint: Outpoint::new(txid, outpoint.vout),
            status: rgb::MiningStatus::Mempool,
            amount: raw_utxo.amount,
            derivation: derive_info,
        }
    }
}

impl From<Utxo> for RawUtxo {
    fn from(utxo: Utxo) -> Self {
        let Utxo {
            outpoint,
            status,
            amount,
            derivation:
                DeriveInfo {
                    terminal: TerminalPath { app, index },
                    tweak,
                },
        } = utxo;

        let mut tap_tweak = none!();
        if let Some(tap) = tweak {
            tap_tweak = Some(tap.to_string())
        }

        let block = match status {
            rgb::MiningStatus::Mempool => 0,
            rgb::MiningStatus::Blockchain(block) => block,
        };

        Self {
            outpoint: outpoint.to_string(),
            block,
            amount,
            terminal: format!("{app}:{index}"),
            tweak: tap_tweak,
        }
    }
}

pub trait RgbMerge<T> {
    fn update(self, rgb_data: &mut T);
}

impl RgbMerge<RawRgbAccount> for RgbAccountV1 {
    fn update(self, rgb_data: &mut RawRgbAccount) {
        for (name, wallet) in self.wallets {
            if let Some(raw_wallet) = rgb_data.wallets.get(&name) {
                let mut raw_wallet = raw_wallet.clone();
                wallet.update(&mut raw_wallet);
                rgb_data.wallets.insert(name, raw_wallet);
            } else {
                rgb_data.wallets.insert(name, RawRgbWallet::from(wallet));
            }
        }
    }
}

impl RgbMerge<RawRgbWallet> for RgbWallet {
    fn update(self, rgb_data: &mut RawRgbWallet) {
        let RgbDescr::Tapret(tapret) = self.descr;

        for (terminal_path, taprets) in tapret.taprets {
            let TerminalPath { app, index } = terminal_path;
            let terminal = format!("{app}:{index}");

            let mut current_taprets = match rgb_data.taprets.get(&terminal) {
                Some(taprets) => taprets.clone(),
                None => vec![],
            };

            let new_taprets: Vec<String> = taprets.into_iter().map(|tap| tap.to_string()).collect();

            for new_tapret in new_taprets {
                if !current_taprets.contains(&new_tapret) {
                    current_taprets.push(new_tapret);
                }
            }

            rgb_data.taprets.insert(terminal, current_taprets);
        }

        for utxo in self.utxos {
            let new_utxo = RawUtxo::from(utxo);
            if !rgb_data.utxos.contains(&new_utxo) {
                rgb_data.utxos.push(new_utxo);
            }
        }
    }
}

impl RgbMerge<RawUtxo> for Utxo {
    fn update(self, rgb_data: &mut RawUtxo) {
        let Utxo {
            status,
            amount,
            derivation: DeriveInfo { tweak, .. },
            ..
        } = self;

        if rgb_data.amount != amount {
            rgb_data.amount = amount;
        }

        if rgb_data.block == 0 {
            rgb_data.block = match status {
                rgb::MiningStatus::Mempool => 0,
                rgb::MiningStatus::Blockchain(block) => block,
            };
        }

        let new_tap = if let Some(new_tap) = tweak {
            Some(new_tap.to_string())
        } else {
            none!()
        };

        if rgb_data.tweak.is_none() {
            rgb_data.tweak = new_tap;
        };
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Display)]
#[display(doc_comments)]
pub struct LocalRgbAccount {
    pub version: Vec<u8>,
    pub rgb_account: RgbAccountV1,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LocalCopyData {
    pub doc: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Display)]
#[display(doc_comments)]
pub struct LocalRgbOffers {
    pub version: Vec<u8>,
    pub rgb_offers: RgbPublicSwaps,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Display)]
#[display(doc_comments)]
pub struct LocalRgbAuctions {
    pub version: Vec<u8>,
    pub rgb_offers: RgbAuctionSwaps,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Display)]
#[display(doc_comments)]
pub struct LocalRgbOfferBid {
    pub version: Vec<u8>,
    pub rgb_bid: RgbBidSwap,
}
