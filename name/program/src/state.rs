use put_program::pubkey::Pubkey;
use borsh::{ BorshDeserialize, BorshSerialize };

/// Tolerance period
pub const GRACE_PERIOD: i64 = 30 * 24 * 60 * 60;
/// Tolerance Charge standard
pub const GRACE_PERIOD_FEE: u128 = 100;
/// Expire period
// pub const EXPIRE_PERIOD: i64 = 365 * 24 * 60 * 60;
pub const EXPIRE_PERIOD: i64 = 10 * 60;
/// Multi-sig interface proposal validity period.
pub const PROPOSAL_EFFECT_PERIOD: u32 = 1 * 24 * 60 * 60;



/// Domain account type
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum AccountType {
    /// TopDomain
    TopDomain,
    /// Domain
    Domain,
    /// DomainResolve
    DomainResolve,
    /// AddressResolve
    AddressResolve,
}

impl Default for AccountType {
    fn default() -> Self {
        return Self::TopDomain
    }
}

impl ToString for AccountType {
    fn to_string(&self) -> String {
        match self {
            AccountType::TopDomain => "TopDomain".to_string(),

            AccountType::Domain => "Domain".to_string(),

            AccountType::DomainResolve => "DomainResolve".to_string(),

            AccountType::AddressResolve => "AddressResolve".to_string(),
        }
    }
}
/// AccountState
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum AccountState {
    /// Uninitialized
    Uninitialized,
    /// Initialized
    Initialized
}

impl ToString for AccountState {
    fn to_string(&self) -> String {
        match self {
            AccountState::Uninitialized => "Uninitialized".to_string(),

            AccountState::Initialized => "Initialized".to_string(),
        }
    }
}

impl Default for AccountState {
    fn default() -> Self {
        return Self::Uninitialized
    }
}

/// Top domain account.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct TopDomainAccount {
    /// Account type
    pub account_type: AccountType, //1 byte
    /// Account state
    pub account_state: AccountState,// 1 byte
    /// Charging rules
    pub rule: [u128; 5],// 80 bytes
    /// The largest space for domain resolution account.
    pub max_space: u16, // 2 bytes
    //todo delete
    // /// Receipt account.
    // pub receipt: Pubkey, // 32 bytes
    /// Domain name.
    pub domain_name: String,
}

/// Oracle account
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct PriceAccount {
    /// origin token
    pub origin: Pubkey, // 32
    /// target token
    pub target: Pubkey, // 32
    /// price
    pub price: u64,   // 16
    /// decimals
    pub decimals: u8,  // 1, default 9
    /// slot
    pub slot: u64,   // 8
    /// bump
    pub bump: u8,    // 1, pda account bump
}

/// Domain account.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DomainAccount {
    /// Account type.
    pub account_type: AccountType,  // 1 byte
    /// Account state.
    pub account_state: AccountState, // 1 byte
    /// Parent Domain.
    pub parent_key: Pubkey, // 32 byte
    /// owner
    pub owner: Pubkey, // 32 byte
    /// Expire time.
    pub expire_time: i64, // 8 byte
    /// The largest space for domain resolution account, from parent.
    pub max_space: u16, // 2 byte
    /// Renewal payment amount index.
    pub fee_index: u32, // 4 byte
    /// Domain name.
    pub domain_name: String,
}



/// Domain resolve account.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DomainResolveAccount {
    /// Account type.
    pub account_type: AccountType,
    /// Account state.
    pub account_state: AccountState,
    /// Parent domain.
    pub parent_key: Pubkey,
    /// Domain name.
    pub domain_name: String,
    /// The value of the domain. For PUT is put account address
    pub value: Option<Vec<u8>>,
}

/// Address resolve account.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct AddressResolveAccount {
    /// Account type.
    pub account_type: AccountType,
    /// Account state.
    pub account_state: AccountState,
    /// Domain name.
    pub domain: String,
}

/// get domain pda.
pub fn get_seeds_and_key(
    program_id: &Pubkey,
    hashed_name: Option<Vec<u8>>, // Hashing is done off-chain
    account_type: AccountType,
    value: Option<Vec<u8>>
) -> Result<(Pubkey, Vec<u8>), String> {
    // let hashed_name: Vec<u8> = hashv(&[(HASH_PREFIX.to_owned() + name).as_bytes()]).0.to_vec();
    let mut seeds_vec: Vec<u8> = vec![];


    if account_type == AccountType::AddressResolve {
        if value.is_none() {
            return Err("AddressResolveAccount must type".to_string())
        }
        seeds_vec.append(&mut value.unwrap())
    } else {
        if hashed_name.is_none() {
            return Err("invalid account type".to_string())
        }
        seeds_vec.append(&mut hashed_name.unwrap())
    }

    let type_data = match account_type {
        AccountType::TopDomain => [0 as u8],
        AccountType::Domain => [1],
        AccountType::DomainResolve => [2],
        AccountType::AddressResolve => [3]
    };

    seeds_vec.push(type_data[0]);

    let (name_account_key, _) =
        Pubkey::find_program_address(&seeds_vec.chunks(32).collect::<Vec<&[u8]>>(), program_id);
    // seeds_vec.push(bump);

    Ok((name_account_key, seeds_vec))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::state::{AccountState, AccountType, TopDomainAccount};
    use borsh::{BorshDeserialize, BorshSerialize};
    use put_program::pubkey::Pubkey;

    struct TestAccount<'a> {
        pub data: Rc<RefCell<&'a mut [u8]>>,
    }

    #[test]
    /// test_top_domain_account_de_and_se.
    fn test_top_domain_account_de_and_se() {
        let account = TopDomainAccount{
            account_type: AccountType::default(),
            account_state: AccountState::default(),
            rule: [0, 1, 2, 3, 4],
            max_space: 0,
            domain_name: "".to_string()
        };
        let mut account_data = borsh::to_vec(&account).unwrap();
        let ret =  account_data.as_mut_slice();

        let test_account = TestAccount{ data: Rc::new(RefCell::new(ret)) };
        // let mut rc_ret = Rc::new(RefCell::new(ret));
        // println!("bytes before {:?}", ret);
        pack_into_slice(&account, &mut test_account.data.borrow_mut());
        // let mut borrow_data  = &mut test_account.data.borrow_mut();
        // println!("bytes {:?}", test_account.data);

        let mut de_account : TopDomainAccount = TopDomainAccount::deserialize (&mut &**test_account.data.borrow_mut()).unwrap();
        println!("account from de {:?}", de_account);
        println!("bytes {:?}", test_account.data.borrow());
        de_account.account_state = AccountState::Initialized;
        de_account.serialize(&mut *test_account.data.borrow_mut());

    }

    fn pack_into_slice(account: &TopDomainAccount, dst: &mut [u8]) {
        let mut slice = dst;
        account.serialize(&mut slice).unwrap();
    }

    #[test]
    fn test_bytes_equal() {
        let a1 = &[1 as u8,2,3,45,5];
        let a2 = [1 as u8,2,3,45,5];
        println!("equal {}",*a1 == a2);
        println!("a1 {:?} ", a1);
    }
}