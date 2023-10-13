use put_program::pubkey::Pubkey;

/// Find proposal account by proposal summary and initiator.
pub fn find_proposal_account(multi_sig_account: &Pubkey, nonce: u64) -> (Pubkey, u8) {

    Pubkey::find_program_address(
        &[
            multi_sig_account.to_bytes().as_ref(),
            &nonce.to_le_bytes()
        ],
        &crate::id())
}


