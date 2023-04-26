use std::str::FromStr;

use bitcoin_30::bip32::ExtendedPubKey;
use bp::dbc::tapret::TapretCommitment;
use commit_verify::mpc::Commitment;
use rgb::{prelude::Resolver, RgbDescr, RgbWallet, Tapret, TerminalPath};

pub fn create_wallet<R: Resolver>(
    xpub: ExtendedPubKey,
    resolver: &mut R,
) -> Result<RgbWallet, anyhow::Error> {
    let descr = RgbDescr::Tapret(Tapret {
        xpub,
        taprets: empty!(),
    });

    let wallet = RgbWallet::with(descr, resolver).expect("");

    Ok(wallet)
}

pub fn save_commit(wallet: &mut RgbWallet, path: TerminalPath, commit: String) {
    let RgbDescr::Tapret(mut tapret) = wallet.descr.clone();
    let mpc = Commitment::from_str(&commit).expect("");
    let tap = TapretCommitment::with(mpc, 0);
    tapret.taprets.insert(path, bset!(tap));
    wallet.descr = RgbDescr::Tapret(tapret);
}
