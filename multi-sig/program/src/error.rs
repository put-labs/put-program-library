//! Error types

use num_derive::FromPrimitive;
use put_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the Token program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum SigError {
    // 0
    /// Account not initialized.
    #[error("Account not initialized.")]
    AccountUnInitialized,

    /// Account has no authority.
    #[error("Account has no authority.")]
    NoAuthority,

    /// Repeat vote.
    #[error("Repeat vote.")]
    RepeatVote,

    /// Invalid threshold.
    #[error("Invalid threshold.")]
    InvalidThreshold,

    /// Too many sig accounts.
    #[error("Too many sig accounts.")]
    TooManyAccounts,
    // 5
    /// Proposal not pass yet.
    #[error("Proposal not pass yet.")]
    NotPass,

    /// Account already exist.
    #[error("Account already exist.")]
    AccountAlreadyExist,

    /// Signers array is full.
    #[error("Signers array is full.")]
    SignersArrayIsFull,

    /// Not a singer.
    #[error("Not a singer.")]
    NotASinger,

    /// Singer already exist.
    #[error("Singer already exist.")]
    SingerAlreadyExist,
    // 10
    /// Singer can not repeat.
    #[error("Singer can not repeat.")]
    SingerCanNotRepeat,

    /// Can not remove.
    #[error("Singers is too less, can not remove.")]
    CanNotRemove,

    /// Invalid proposal account.
    #[error("Invalid proposal account.")]
    InvalidProposalAccount,

    /// Proposal can not be close cause unexpired
    #[error("Proposal can not be close cause unexpired")]
    CanNotClose,

    /// Threshold not change.
    #[error("Threshold not change.")]
    ThresholdNotChange,

    /// Cannot unfrozen, account not frozen.
    #[error("Cannot unfrozen, account not frozen")]
    CannotUnfrozen,

    /// Summary unmatched with before.
    #[error("Summary unmatched with before")]
    SummaryUnmatched,

    /// Singers must greater than 0.
    #[error("Singers must greater than 0")]
    SignersNotBeEmpty,

    /// No authority to close.
    #[error("No authority to close")]
    NoAuthToClose
}
impl From<SigError> for ProgramError {
    fn from(e: SigError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for SigError {
    fn type_of() -> &'static str {
        "SigError"
    }
}
