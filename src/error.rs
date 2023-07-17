use thiserror::Error;

use crate::{bitcoin::BitcoinKeysError, rgb::IssueError};

#[derive(Error, Debug, Display)]
#[display(doc_comments)]
pub enum BitMaskCoreError {
    /// Bitcoin Keys Error
    BitcoinKeysError(#[from] BitcoinKeysError),
    /// RGB Issuer Operation Error
    RgbIssueError(#[from] IssueError),
}
