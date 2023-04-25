use amplify::hex::ToHex;
use psbt::ProprietaryKey;
use rgb::{
    containers::Transfer,
    psbt::{
        DbcPsbtError, TapretKeyError, PSBT_OUT_TAPRET_COMMITMENT, PSBT_OUT_TAPRET_HOST,
        PSBT_TAPRET_PREFIX,
    },
};
use wallet::psbt::Psbt;

pub fn extract_psbt_commit(mut psbt: Psbt) -> Result<String, DbcPsbtError> {
    let (_, output) = psbt
        .outputs
        .iter_mut()
        .enumerate()
        .find(|(_, output)| {
            output.proprietary.contains_key(&ProprietaryKey {
                prefix: PSBT_TAPRET_PREFIX.to_vec(),
                subtype: PSBT_OUT_TAPRET_HOST,
                key: vec![],
            })
        })
        .ok_or(DbcPsbtError::NoHostOutput)
        .expect("");

    let commit_vec = output.proprietary.get(&ProprietaryKey {
        prefix: PSBT_TAPRET_PREFIX.to_vec(),
        subtype: PSBT_OUT_TAPRET_COMMITMENT,
        key: vec![],
    });

    match commit_vec {
        Some(commit) => Ok(commit.to_hex()),
        _ => Err(DbcPsbtError::TapretKey(TapretKeyError::InvalidProof)),
    }
}

pub fn extract_transfer_commit(mut transfer: Transfer) -> Result<String, DbcPsbtError> {
    Ok("".to_string())
}
