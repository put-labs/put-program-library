use put_program::pubkey::{Pubkey, PUBKEY_BYTES};
use borsh::{ BorshDeserialize, BorshSerialize };
use put_program::entrypoint::ProgramResult;
use put_program::program_memory::put_memcmp;
use crate::error::SigError;

/// Transaction validity period.
pub const PROPOSAL_EFFECT_PERIOD: i64 = 1 * 1 * 10 * 60;
/// Maximum number of signatures.
pub const MAX_MULTI_SIG_ACCOUNTS: usize = 10;
/// empty check closure
pub const EMPTY_CHECK: fn(&MultiSigAccount) -> ProgramResult = |_ :&MultiSigAccount| -> ProgramResult {
    Ok(())
};

/// Account state.
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum AccountState {
    /// Uninitialized
    Uninitialized,
    /// Initialized
    Initialized,
    /// Proposal verified, for proposal account
    Verified
}

impl Default for AccountState {
    fn default() -> Self {
        AccountState::Uninitialized
    }
}

impl ToString for AccountState {
    fn to_string(&self) -> String {
        return match self {
            AccountState::Uninitialized => {
                "Uninitialized".to_string()
            }
            AccountState::Initialized => {
                "Initialized".to_string()
            }
            AccountState::Verified => {
                "Verified".to_string()
            }
        }
    }
}

/// Whether the space is occupied.
pub type IsOccupied = bool;
/// Is the space already ticketed.
pub type IsVoted = bool;

/// Multi sig account.
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct MultiSigAccount {
    /// Account state.
    pub account_state :AccountState,
    /// All valid signers.
    pub accounts: [(Pubkey, IsOccupied); MAX_MULTI_SIG_ACCOUNTS],
    /// threshold, value 1-100
    pub threshold: u8,
    /// Nonce, record current MultiSigAccount created proposal count
    pub nonce: u64,
}

/// impl multi_sig_account
impl MultiSigAccount {
    /// is signer
    pub fn is_signer(&self, src_account: &Pubkey) -> bool {
        self
            .accounts
            .iter()
            .find(|(account,is_occupied)| {
                if !is_occupied {
                    return false
                }
                return pubkeys_equal(src_account,account)
            }).is_some()
    }

    /// has_vacancy_and_get_suitable_index
    pub fn has_vacancy_and_get_suitable_index(&self) -> (bool, usize) {
        for (index, x) in self.accounts.iter().enumerate() {
            if !x.1 {
                return (true, index)
            }
        }
        (false, 0)
    }

    /// get_valid_singers_count
    pub fn get_valid_singers_count(&self) -> usize {
        let mut len = 0;
        self.accounts.iter().for_each(|x| {
           if x.1 {
              len += 1;
           }
        });
        len
    }

    /// is_signer_and_get_seat
    pub fn is_signer_and_get_seat(&self, src_account: &Pubkey) -> (bool, usize) {
        for (index, x) in self.accounts.iter().enumerate() {
            if x.1 && pubkeys_equal(src_account, &x.0) {
                return (true, index)
            }
        }
        (false, 0)
    }
    /// get_valid_tickets_count
    pub fn get_valid_tickets_count(&self) -> usize {
        let mut ret = 0;
        for (_, x) in self.accounts.iter().enumerate() {
            if x.1 {
                ret += 1
            }
        }
        ret
    }

    /// Add new signer.
    pub fn add_new(&mut self, new_signer: Pubkey) -> ProgramResult {
        let (has_seat, index) = self.has_vacancy_and_get_suitable_index();
        if !has_seat {
            return Err(SigError::SignersArrayIsFull.into())
        }
        self.accounts[index] = (new_signer, true);
        ProgramResult::Ok(())
    }
}

/// ProposalAccount
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct ProposalAccount {
    /// AccountState
    pub account_state: AccountState,
    /// Tickets, Agreed singers.
    pub tickets: [(Pubkey, IsVoted); MAX_MULTI_SIG_ACCOUNTS],
    /// The Multi sig account.
    pub parent: Pubkey,
    /// Tx expired time.
    pub tx_expired_time: i64,
    /// Proposal initiation account.
    pub initiator: Pubkey,
    /// Proposal summary
    pub summary: String,
    /// Nonce, from multi sig account, represent current proposal id
    pub nonce: u64,
}

impl ProposalAccount {
    /// is_voted_and_get_suitable_place
    pub fn is_voted_and_get_suitable_place(&self, vote_account: &Pubkey) -> (bool, usize) {
        for (index, x) in self.tickets.iter().enumerate() {
            if !x.1 {
                // find a ticket seat.
                return (false, index)
            }
            if pubkeys_equal(&x.0, vote_account) {
                return (true, 0)
            }
        }
        // All votes have been cast.
        (true, 0)
    }

    /// Count tickets
    pub fn count_tickets(&self) -> usize {
       let mut ret = 0;
       self
           .tickets
           .iter()
           .for_each(|x| {
               if x.1 {
                   ret += 1;
               }
           });
       ret
    }
}

/// Checks two pubkeys for equality in a computationally cheap way using
/// `sol_memcmp`
pub fn pubkeys_equal(a: &Pubkey, b: &Pubkey) -> bool {
    put_memcmp(a.as_ref(), b.as_ref(), PUBKEY_BYTES) == 0
}

