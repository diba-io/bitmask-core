use thiserror::Error;

use crate::{bitcoin::BitcoinKeysError, rgb::IssueError};

#[derive(Error, Debug)]
pub enum BitMaskCoreError {
    /// Bitcoin Keys Error
    #[error(transparent)]
    BitcoinKeysError(#[from] BitcoinKeysError),
    /// RGB Issuer Operation Error
    #[error(transparent)]
    RgbIssueError(#[from] IssueError),
}
