#![deny(missing_docs)]
#![cfg_attr(not(test), forbid(unsafe_code))]

//! An ERC20-like Token program for the put blockchain

extern crate core;

pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

// Export current sdk types for downstream users building with a different sdk version
pub use put_program;
use put_program::{entrypoint::ProgramResult, program_error::ProgramError, pubkey::Pubkey};

put_program::declare_id!("An2DRyUtGBKYioLhHJEQ3nPcGgzzRJQ8vgdhyjdtC14H");

/// Checks that the supplied program ID is the correct one for SPL-token
pub fn check_program_account(nft_program_id: &Pubkey) -> ProgramResult {
    if nft_program_id != &id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}
