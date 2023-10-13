//! Error types

use num_derive::FromPrimitive;
use put_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the Token program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum NameError {
    // 0
    /// Invalid domain name format.
    #[error("Invalid domain name format.")]
    InvalidNameFormat,

    /// Parent domain expired.
    #[error("Parent domain expired.")]
    DomainExpired,

    /// Account not exist.
    #[error("Account not exist.")]
    AccountNotExist,

    /// Invalid proposal account.
    #[error("Invalid proposal account.")]
    InvalidProposalAccount,

    /// Invalid multi sig account.
    #[error("Invalid multi sig account.")]
    InvalidMultiSigAccount,

    // 5
    /// Repeat unbind value.
    #[error("Repeat unbind value.")]
    RepeatUnbind,

    /// Invalid receipt account.
    #[error("Invalid receipt account.")]
    InvalidReceiptAccount,

    /// Invalid value length.
    #[error("Invalid value length.")]
    InvalidValueLen

}
impl From<NameError> for ProgramError {
    fn from(e: NameError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for NameError {
    fn type_of() -> &'static str {
        "NameError"
    }
}
