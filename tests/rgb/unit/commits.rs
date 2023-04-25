use bitmask_core::rgb::{commits::extract_psbt_commit, invoice::pay_invoice};
use rgb::persistence::Stock;

use crate::rgb::utils::{create_fake_contract, create_fake_invoice, create_fake_psbt};

#[tokio::test]
async fn allow_extract_commit_from_psbt() -> anyhow::Result<()> {
    let mut stock = Stock::default();
    let psbt = create_fake_psbt();

    let contract_id = create_fake_contract(&mut stock);

    let seal = "tapret1st:ed823b41d8b9309933826b18e4af530363b359f05919c02bbe72f28cec6dec3e:0";
    let invoice = create_fake_invoice(contract_id, seal, &mut stock);

    let result = pay_invoice(invoice.to_string(), psbt.to_string(), &mut stock);
    assert!(result.is_ok());

    let (psbt, _) = result.unwrap();

    let commit = extract_psbt_commit(psbt);
    assert!(commit.is_ok());
    Ok(())
}

#[tokio::test]
async fn allow_extract_commit_from_transfer() -> anyhow::Result<()> {
    Ok(())
}
