use anyhow::{Ok, Result};
use bitmask_core::{
    lightning::CreateWalletRes,
    operations::lightning::{
        auth, create_invoice, create_wallet, decode_invoice, get_balance, get_txs, pay_invoice,
        PayInvoiceMessage, Tx,
    },
};
use log::info;

async fn new_wallet() -> Result<CreateWalletRes> {
    // we generate a random string to be used as username and password
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf)?;
    let s = buf.map(|d| format!("{d:02x}")).join("");

    let res = create_wallet(&s, &s).await?;

    Ok(res)
}

#[tokio::test]
pub async fn create_wallet_test() -> Result<()> {
    pretty_env_logger::init();
    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletRes::Username { username } = res {
        uname = username;
    }

    assert!(uname.len() == 16);

    Ok(())
}

#[tokio::test]
pub async fn auth_test() -> Result<()> {
    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletRes::Username { username } = res {
        uname = username;
    }
    let tokens = auth(&uname, &uname).await?;
    assert!(tokens.refresh.len() > 1 && tokens.token.len() > 1);

    Ok(())
}

#[tokio::test]
pub async fn create_decode_invoice_test() -> Result<()> {
    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletRes::Username { username } = res {
        uname = username;
    }
    let tokens = auth(&uname, &uname).await?;
    let invoice = create_invoice("testing create_invoice", "0.000099", &tokens.token).await?;
    // let decoded_invoice = decode_invoice(&invoice, &tokens.token).await?;
    info!("invoice: {invoice:?}");
    // info!("decoded_invoice: {decoded_invoice:#?}");
    // assert_eq!(decoded_invoice.num_satoshis, "333");

    Ok(())
}

// #[tokio::test]
// pub async fn get_balance_test() -> Result<()> {
//     let creds = create_wallet().await?;
//     let tokens = auth(&creds.login, &creds.password).await?;
//     let balance = get_balance(&tokens.access_token).await?;
//     assert_eq!(balance.btc.available_balance, 0);
//     info!("balance: {balance:#?}");

//     Ok(())
// }

// #[tokio::test]
// pub async fn pay_invoice_error_test() -> Result<()> {
//     // We create user Alice
//     let alice_creds = create_wallet().await?;
//     let alice_tokens = auth(&alice_creds.login, &alice_creds.password).await?;
//     // Alice invoice
//     let invoice =
//         create_invoice("testing pay alice invoice", 33, &alice_tokens.access_token).await?;
//     // We create user Bob
//     let bob_creds = create_wallet().await?;
//     let bob_tokens = auth(&bob_creds.login, &bob_creds.password).await?;
//     // We try to pay alice invoice from bob, which have balance = 0
//     let response = pay_invoice(&invoice, &bob_tokens.access_token).await?;
//     assert_eq!(
//         response,
//         PayInvoiceMessage::PayInvoiceError {
//             error: true,
//             code: 2,
//             message:
//                 "not enough balance. Make sure you have at least 1% reserved for potential fees"
//                     .to_string(),
//         }
//     );
//     info!("pay_invoice: {response:#?}");

//     Ok(())
// }

// #[tokio::test]
// pub async fn get_txs_test() -> Result<()> {
//     let creds = create_wallet().await?;
//     let tokens = auth(&creds.login, &creds.password).await?;
//     let txs: Vec<Tx> = get_txs(&tokens.access_token, 0, 0).await?;
//     assert_eq!(txs.len(), 0);

//     info!("get_txs_test: {txs:#?}");

//     Ok(())
// }
