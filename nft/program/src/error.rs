//! Error types

use num_derive::FromPrimitive;
use put_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the Token program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum TokenError {
    // 0
    /// Account not associated with this Mint.
    #[error("Account not associated with this Mint")]
    MintMismatch,

    /// Owner does not match.
    #[error("Owner not matched")]
    OwnerMismatch,

    /// The account cannot be initialized because it is already being used.
    #[error("Already in use")]
    AlreadyInUse,

    /// State is uninitialized.
    #[error("State is uninitialized")]
    UninitializedState,

    /// Operation overflowed
    #[error("Operation overflowed")]
    Overflow,

    // 5
    /// Account is frozen; all account operations will fail
    #[error("Account is frozen")]
    AccountFrozen,

    /// Token of mint already reach max_supply count
    #[error("token of mint already reach max_supply count")]
    AlreadyReachMaxMintNum,

    /// Len of name is gt max_len
    #[error("Len of name is gt max_len")]
    NameIsTooLen,

    /// Len of symbol is gt max_len
    #[error("len of symbol is gt max_len")]
    SymbolIsTooLen,

    /// Len of uri is gt max_len
    #[error("Len of uri is gt max_len")]
    UriIsTooLen,

    // 10
    /// Authority Mismatched
    #[error("Authority Mismatched")]
    AuthorityMismatched,

    /// Nft account already frozen
    #[error("Nft account already frozen")]
    AlreadyFrozen,

    /// Set authority with same account
    #[error("Set authority with same account")]
    SameAuthority,

    /// Thaw a unfrozen nft
    #[error("Thaw a unfrozen nft")]
    ThawUnfrozen
}

impl From<TokenError> for ProgramError {
    fn from(e: TokenError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for TokenError {
    fn type_of() -> &'static str {
        "TokenError"
    }
}
