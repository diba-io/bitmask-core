use thiserror::Error;

use crate::{bitcoin::BitcoinKeysError, rgb::IssueError};

#[derive(Error, Debug, Display)]
#[display(doc_comments)]
pub enum BitMaskCoreError {
    /// Bitcoin Keys Error
    #[error(transparent)]
    BitcoinKeysError(#[from] BitcoinKeysError),
    /// RGB Issuer Operation Error
    #[error(transparent)]
    RgbIssueError(#[from] IssueError),
}
