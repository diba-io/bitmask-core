#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use rgbstd::{
    interface::{rgb20_stl, rgb21_stl, rgb25_stl, LIB_ID_RGB20, LIB_ID_RGB21, LIB_ID_RGB25},
    stl::{rgb_contract_stl, rgb_std_stl, LIB_ID_RGB_CONTRACT, LIB_ID_RGB_STD},
};

#[tokio::test]
async fn check_rgb20_stl() -> Result<()> {
    let shema = rgb20_stl();
    assert_eq!(LIB_ID_RGB20, shema.id().to_string());
    Ok(())
}

#[tokio::test]
async fn check_rgb21_stl() -> Result<()> {
    let shema = rgb21_stl();
    assert_eq!(LIB_ID_RGB21, shema.id().to_string());
    Ok(())
}

#[tokio::test]
async fn check_rgb25_stl() -> Result<()> {
    let shema = rgb25_stl();
    assert_eq!(LIB_ID_RGB25, shema.id().to_string());
    Ok(())
}

#[tokio::test]
async fn check_rgbstd_stl() -> Result<()> {
    let shema = rgb_std_stl();
    assert_eq!(LIB_ID_RGB_STD, shema.id().to_string());
    Ok(())
}

#[tokio::test]
async fn check_rgbcontract_stl() -> Result<()> {
    let shema = rgb_contract_stl();
    assert_eq!(LIB_ID_RGB_CONTRACT, shema.id().to_string());
    Ok(())
}
