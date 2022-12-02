use anyhow::{Ok, Result};
use bitmask_core::{
    lightning::CreateWalletResponse,
    operations::lightning::{
        auth, create_invoice, create_wallet, decode_invoice, get_balance, get_txs, pay_invoice,
        Transaction,
    },
};

async fn new_wallet() -> Result<CreateWalletResponse> {
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
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }

    assert!(uname.len() == 16);

    Ok(())
}

#[tokio::test]
pub async fn auth_test() -> Result<()> {
    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
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
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let description = "testing create_invoice";
    let amt = 99;
    let amt_milli: u64 = 99 * 1000;
    let tokens = auth(&uname, &uname).await?;
    let invoice = create_invoice(description, amt, &tokens.token).await?;
    let payment_request = invoice.payment_request.unwrap();
    let decoded_invoice = decode_invoice(&payment_request)?;
    let invoice_amt = decoded_invoice.amount_milli_satoshis().unwrap();

    assert_eq!(amt_milli, invoice_amt);

    Ok(())
}

#[tokio::test]
pub async fn get_balance_test() -> Result<()> {
    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let tokens = auth(&uname, &uname).await?;
    let balances = get_balance(&tokens.token).await?;

    assert_eq!(balances.len(), 1);
    if let Some(b) = balances.get(0) {
        assert_eq!(b.balance, "0");
        assert_eq!(b.currency, "BTC");
    }

    Ok(())
}

#[tokio::test]
pub async fn pay_invoice_error_test() -> Result<()> {
    // We create user Alice
    let res = new_wallet().await?;
    let mut alice = String::new();
    if let CreateWalletResponse::Username { username } = res {
        alice = username;
    }
    let alice_tokens = auth(&alice, &alice).await?;
    // Alice invoice
    let invoice = create_invoice("testing pay alice invoice", 33, &alice_tokens.token).await?;
    // We create user Bob
    let res = new_wallet().await?;
    let mut bob = String::new();
    if let CreateWalletResponse::Username { username } = res {
        bob = username;
    }
    let bob_tokens = auth(&bob, &bob).await?;
    // We try to pay alice invoice from bob, which have balance = 0
    let response = pay_invoice(&invoice.payment_request.unwrap(), &bob_tokens.token).await?;
    assert!(!response.success);

    Ok(())
}

#[tokio::test]
pub async fn get_txs_test() -> Result<()> {
    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let tokens = auth(&uname, &uname).await?;
    let txs: Vec<Transaction> = get_txs(&tokens.token).await?;
    assert_eq!(txs.len(), 0);

    Ok(())
}
