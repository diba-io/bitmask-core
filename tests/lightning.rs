#![cfg(not(target_arch = "wasm32"))]

use anyhow::{Ok, Result};
use bitmask_core::{
    info,
    lightning::{AuthResponse, CreateWalletResponse},
    operations::lightning::{
        auth, check_payment, create_invoice, create_wallet, decode_invoice, get_balance, get_txs,
        pay_invoice, Transaction,
    },
    util::init_logging,
};
use std::{thread, time};

async fn new_wallet() -> Result<CreateWalletResponse> {
    // we generate a random string to be used as username and password
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf)?;
    let s = buf.map(|d| format!("{d:02x}")).join("");
    // We put to sleep the test to avoid hit too fast the API
    thread::sleep(time::Duration::from_secs(1));
    let res = create_wallet(&s, &s).await?;

    Ok(res)
}

#[tokio::test]
pub async fn create_wallet_test() -> Result<()> {
    init_logging("lightning=debug");

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
    init_logging("lightning=warn");

    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let response = auth(&uname, &uname).await?;
    thread::sleep(time::Duration::from_secs(1));
    if let AuthResponse::Result { refresh, token } = response {
        assert!(refresh.len() > 1 && token.len() > 1);
    }

    Ok(())
}

#[tokio::test]
pub async fn auth_failed_test() -> Result<()> {
    init_logging("lightning=warn");

    let response = auth("fake_username", "fake_password").await?;
    if let AuthResponse::Error { error } = response {
        assert_eq!(error, "UserDoesNotExist");
    }

    Ok(())
}

#[tokio::test]
pub async fn create_decode_invoice_test() -> Result<()> {
    init_logging("lightning=warn");

    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let description = "testing create_invoice";
    let amt = 99;
    let amt_milli: u64 = 99 * 1000;
    let response = auth(&uname, &uname).await?;
    thread::sleep(time::Duration::from_secs(1));
    if let AuthResponse::Result { refresh: _, token } = response {
        let invoice = create_invoice(description, amt, &token).await?;
        let payment_request = invoice.payment_request.unwrap();
        let decoded_invoice = decode_invoice(&payment_request)?;
        let invoice_amt = decoded_invoice.amount_milli_satoshis().unwrap();

        assert_eq!(amt_milli, invoice_amt);
    }

    Ok(())
}

#[tokio::test]
pub async fn get_balance_test() -> Result<()> {
    init_logging("lightning=warn");

    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let response = auth(&uname, &uname).await?;
    thread::sleep(time::Duration::from_secs(1));
    if let AuthResponse::Result { refresh: _, token } = response {
        let accounts = get_balance(&token).await?;
        let btc_account = accounts.get(0).unwrap();
        assert_eq!(btc_account.balance, "0");
    }

    Ok(())
}

#[tokio::test]
pub async fn pay_invoice_error_test() -> Result<()> {
    init_logging("tests=debug");

    info!("We create user Alice");
    let res = new_wallet().await?;
    let mut alice = String::new();
    if let CreateWalletResponse::Username { username } = res {
        alice = username;
    }
    let alice_response = auth(&alice, &alice).await?;
    thread::sleep(time::Duration::from_secs(1));
    if let AuthResponse::Result { refresh: _, token } = alice_response {
        info!("Alice invoice");
        let invoice = create_invoice("testing pay alice invoice", 33, &token).await?;
        info!("We create user Bob");
        let res = new_wallet().await?;
        let mut bob = String::new();
        if let CreateWalletResponse::Username { username } = res {
            bob = username;
        }
        let bob_response = auth(&bob, &bob).await?;
        thread::sleep(time::Duration::from_secs(1));
        if let AuthResponse::Result { refresh: _, token } = bob_response {
            info!("We try to pay alice invoice from bob, which have balance = 0");
            let response = pay_invoice(&invoice.payment_request.unwrap(), &token).await?;
            assert!(!response.success);
        }
    }

    Ok(())
}

#[tokio::test]
pub async fn get_txs_test() -> Result<()> {
    init_logging("lightning=warn");

    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let response = auth(&uname, &uname).await?;
    thread::sleep(time::Duration::from_secs(1));
    if let AuthResponse::Result { refresh: _, token } = response {
        let txs: Vec<Transaction> = get_txs(&token).await?;
        assert_eq!(txs.len(), 0);
    }

    Ok(())
}

#[tokio::test]
pub async fn check_payment_test() -> Result<()> {
    init_logging("lightning=warn");

    let res = new_wallet().await?;
    let mut uname = String::new();
    if let CreateWalletResponse::Username { username } = res {
        uname = username;
    }
    let response = auth(&uname, &uname).await?;
    thread::sleep(time::Duration::from_secs(1));
    if let AuthResponse::Result { refresh: _, token } = response {
        let invoice = create_invoice("payment description", 99, &token).await?;
        let payment_request = invoice.payment_request.unwrap();
        let decoded_invoice = decode_invoice(&payment_request)?;
        let payment_hash = decoded_invoice.payment_hash().to_string();
        let is_paid = check_payment(&payment_hash).await?;

        assert!(!is_paid);
    }

    Ok(())
}
