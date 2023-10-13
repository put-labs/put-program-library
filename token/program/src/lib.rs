#![deny(missing_docs)]
#![forbid(unsafe_code)]
// #![feature(round_char_boundary)]

//! An ERC20-like Token program for the PUT blockchain

pub mod error;
pub mod instruction;
pub mod native_mint;
pub mod native_mint_info;
pub mod processor;
pub mod state;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

// Export current sdk types for downstream users building with a different sdk version
pub use put_program;
use put_program::{entrypoint::ProgramResult, program_error::ProgramError, pubkey::Pubkey};

/// Convert the UI representation of a token amount (using the decimals field defined in its mint)
/// to the raw amount
pub fn ui_amount_to_amount(inte: u128, frac: f64, decimals: u8) -> u128 {
    inte * 10_usize.pow(decimals as u32) as u128 + (frac * 10_usize.pow(decimals as u32) as f64) as u128
}

/// Convert a raw amount to its UI representation (using the decimals field defined in its mint)
pub fn amount_to_ui_amount(amount: u128, decimals: u8) -> String {
    let decimals = decimals as usize;
    if decimals > 0 {
        // Left-pad zeros to decimals + 1, so we at least have an integer zero
        let mut s = format!("{:01$}", amount, decimals + 1);
        // Add the decimal point (Sorry, "," locales!)
        s.insert(s.len() - decimals, '.');
        let zeros_trimmed = s.trim_end_matches('0');
        s = zeros_trimmed.trim_end_matches('.').to_string();
        s
    } else {
        amount.to_string()
    }
}

put_program::declare_id!("PutToken11111111111111111111111111111111111");
// put_program::declare_id!("3LbxkbtrnKaUeqYLpmXx8QNUAYfw8PUqcjib4xLdvGyJ");


/// Checks that the supplied program ID is the correct one for PPL-token
pub fn check_program_account(ppl_token_program_id: &Pubkey) -> ProgramResult {
    if ppl_token_program_id != &id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_to_ui_amount() {

        assert_eq!("340282366920938463463.374607431768211455",
        amount_to_ui_amount(340282366920938463463374607431768211455,18)
        );
        assert_eq!("340282366920938463463374607431.768211455",
        amount_to_ui_amount(340282366920938463463374607431768211455,9)
        );
        assert_eq!("340282366920938463463374607431.000011455",
        amount_to_ui_amount(340282366920938463463374607431000011455,9)
        );

    }
}
