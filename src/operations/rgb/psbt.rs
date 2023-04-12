use bitcoin_blockchain::locks::LockTime;
use bitcoin_scripts::PubkeyScript;
use miniscript_crate::Descriptor;
use wallet::psbt::Psbt;
use wallet::{
    descriptors::InputDescriptor,
    hd::{DerivationAccount, UnhardenedIndex},
    onchain::ResolveTx,
    psbt::{ProprietaryKeyDescriptor, ProprietaryKeyError, ProprietaryKeyLocation},
};

pub fn create_psbt(
    descriptor: &Descriptor<DerivationAccount>,
    lock_time: LockTime,
    inputs: Vec<InputDescriptor>,
    outputs: Vec<(PubkeyScript, u64)>,
    proprietary_keys: Vec<ProprietaryKeyDescriptor>,
    change_index: UnhardenedIndex,
    fee: u64,
    tx_resolver: &impl ResolveTx,
) -> Result<Psbt, ProprietaryKeyError> {
    let mut psbt = Psbt::construct(
        &descriptor,
        &inputs,
        &outputs,
        change_index,
        fee,
        tx_resolver,
    )
    .expect("");

    psbt.fallback_locktime = Some(lock_time);

    for key in proprietary_keys {
        match key.location {
            ProprietaryKeyLocation::Input(pos) if pos as usize >= psbt.inputs.len() => {
                return Err(ProprietaryKeyError::InputOutOfRange(pos, psbt.inputs.len()).into())
            }
            ProprietaryKeyLocation::Output(pos) if pos as usize >= psbt.outputs.len() => {
                return Err(ProprietaryKeyError::OutputOutOfRange(pos, psbt.inputs.len()).into())
            }
            ProprietaryKeyLocation::Global => {
                psbt.proprietary.insert(
                    key.clone().into(),
                    key.value.as_ref().cloned().unwrap_or_default(),
                );
            }
            ProprietaryKeyLocation::Input(pos) => {
                psbt.inputs[pos as usize].proprietary.insert(
                    key.clone().into(),
                    key.value.as_ref().cloned().unwrap_or_default(),
                );
            }
            ProprietaryKeyLocation::Output(pos) => {
                psbt.outputs[pos as usize].proprietary.insert(
                    key.clone().into(),
                    key.value.as_ref().cloned().unwrap_or_default(),
                );
            }
        }
    }

    Ok(psbt)
}
