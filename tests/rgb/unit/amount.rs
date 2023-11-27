#![cfg(not(target_arch = "wasm32"))]
use bitmask_core::rgb::structs::ContractAmount;
#[tokio::test]
async fn create_contract_amount() -> anyhow::Result<()> {
    let amount = ContractAmount::new(9900, 2);
    assert_eq!(amount.int, 99);
    assert_eq!(amount.fract, 0);
    assert_eq!(amount.to_value(), 9900);
    assert_eq!(amount.to_string(), "99.00");

    let amount = ContractAmount::new(10000, 2);
    assert_eq!(amount.int, 100);
    assert_eq!(amount.fract, 0);
    assert_eq!(amount.to_value(), 10000);
    assert_eq!(amount.to_string(), "100.00");

    let amount = ContractAmount::with(4000, 0, 2);
    assert_eq!(amount.int, 4000);
    assert_eq!(amount.fract, 0);
    assert_eq!(amount.to_value(), 400000);
    assert_eq!(amount.to_string(), "4000.00");

    let amount = ContractAmount::with(1000, 1, 2);
    assert_eq!(amount.int, 1000);
    assert_eq!(amount.fract, 1);
    assert_eq!(amount.to_value(), 100001);
    assert_eq!(amount.to_string(), "1000.01");

    Ok(())
}
