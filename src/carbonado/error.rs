use thiserror::Error;

#[derive(Error, Debug, Display)]
#[display(doc_comments)]
pub enum CarbonadoError {
    /// std io error: {0}
    StdIoError(#[from] std::io::Error),
    /// std io error: {0}
    StdStrUtf8Error(#[from] std::str::Utf8Error),
    /// Error decoding hexadecimal-encoded string: {0}
    HexDecodeError(#[from] hex::FromHexError),
    /// Error decoding base64-encoded string: {0}
    Base64DecodeError(#[from] base64::DecodeError),
    /// Error creating Nostr private key: {0}
    NostrPrivateKey(#[from] nostr_sdk::secp256k1::Error),
    /// General Carbonado error: {0}
    CarbonadoError(#[from] carbonado::error::CarbonadoError),
    /// General Carbonado error: {0}
    SerdeJsonError(#[from] serde_json::Error),
    /// JS Error: {0}
    #[cfg(target_arch = "wasm32")]
    JsError(#[from] gloo_utils::errors::JsError),
    /// Serde WASM Error: {0}
    #[cfg(target_arch = "wasm32")]
    SerdeWasm(#[from] serde_wasm_bindgen::Error),
    /// All endpoints failed error
    AllEndpointsFailed,
    /// Debug: {0}
    Debug(String),
    /// No secret key available in memory
    NoSecretKey,
}
