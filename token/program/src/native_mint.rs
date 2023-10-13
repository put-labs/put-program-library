//! The Mint that represents the native token

/// There are 10^9 lamports in one PUT
pub const DECIMALS: u8 = 9;

/// The symbol in PPL-Toekn PUT
pub const SYMBOL: &str = "WPUT";
/// The name in PPL-Toekn PUT
pub const NAME: &str = "Wrap PUT";
/// The icon url in PPL-Toekn PUT
pub const ICON: &str = "https://static.put.com/icon/put.svg";

// The Mint for native PUT Token accounts
put_program::declare_id!("Put1111111111111111111111111111111111111111");

#[cfg(test)]
mod tests {
    use super::*;
    use put_program::native_token::*;

    #[test]
    fn test_decimals() {
        // assert!(
        //     (lamports_to_put(42) - crate::amount_to_ui_amount(42, DECIMALS)).abs() < f64::EPSILON
        // );
        assert_eq!(
            put_to_lamports("42.".to_string()),
            crate::ui_amount_to_amount(42,0.0, DECIMALS)
        );
    }
}
