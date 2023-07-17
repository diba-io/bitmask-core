use thiserror::Error;

#[derive(Error, Debug, Display)]
#[display(doc_comments)]
pub enum CarbonadoError {
    /// std io error
    StdIoError(#[from] std::io::Error),
    /// Error decoding hexadecimal-encoded string
    HexDecodeError(#[from] hex::FromHexError),
    /// Error decoding base64-encoded string
    Base64DecodeError(#[from] base64::DecodeError),
    /// Error creating Nostr private key
    NostrPrivateKey(#[from] nostr_sdk::secp256k1::Error),
    /// General Carbonado error
    CarbonadoError(#[from] carbonado::error::CarbonadoError),
}
