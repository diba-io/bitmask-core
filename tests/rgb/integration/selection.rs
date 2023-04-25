use bitmask_core::util::init_logging;

#[tokio::test]
async fn issue_contract_test() -> anyhow::Result<()> {
    init_logging("rgb_issue=warn");

    let mnemonic_phrase =
        "ordinary crucial edit settle pencil lion appear unlock left fly century license";
    let seed_password = "";
    let vault_data = bitmask_core::bitcoin::save_mnemonic(mnemonic_phrase, seed_password).await?;

    println!("{:#?}", vault_data);

    Ok(())
}
