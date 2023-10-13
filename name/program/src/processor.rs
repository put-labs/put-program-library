use std::ops::{Div};
use std::slice::Iter;
use put_program::account_info::{AccountInfo, next_account_info};
use put_program::entrypoint::ProgramResult;
use put_program::hash::{hashv};
use put_program::{clock, msg, system_instruction, system_program};
use put_program::program_error::ProgramError;
use put_program::pubkey::{Pubkey, PUBKEY_BYTES};
use crate::instruction::{is_valid_domain, NameInstruction};
use crate::state::{AccountState, AccountType, AddressResolveAccount, DomainAccount, DomainResolveAccount, EXPIRE_PERIOD, get_seeds_and_key, GRACE_PERIOD, GRACE_PERIOD_FEE, PriceAccount, PROPOSAL_EFFECT_PERIOD, TopDomainAccount};
use borsh::{BorshDeserialize, BorshSerialize};
use put_program::program::{invoke, invoke_signed};
use put_program::program_memory::{put_memcmp, put_memset};
use put_program::rent::Rent;
use put_program::sysvar::Sysvar;
use ppl_sig::instruction::{create_proposal_account, verify};
use ppl_sig::state::{ProposalAccount};
use crate::error::NameError;
use crate::{multi_sig_account_inline, oracle_program, usdt_token_account};

/// 1 put to lamports
const PUT_BASE: f64 = 1000000000_f64;

/// Program state handler.
pub struct Processor {}

impl Processor {

    /// create top domain, a multi sig interface.
    fn _create_top_domain(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, domain_name: String, rule: [u128; 5], max_space: u16) -> ProgramResult{
        let top_domain_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let multi_sig_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;
        let _multi_sig_program_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        // Verify the domain name format.
        if !is_valid_domain(&domain_name) {
            msg!("invalid domain name format");
            return Err(NameError::InvalidNameFormat.into())
        }

        // Domain account check.
        let hash = hashv(&[domain_name.as_bytes()]);
        let (account_pub_key,plain_seeds) = get_seeds_and_key(program_id,Some(hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();
        if top_domain_account.key != &account_pub_key {
            msg!("unmatched domain name with domain account");
            return Err(ProgramError::InvalidArgument)
        }
        // Account initialization check.
        let mut top_domain_data_obj : TopDomainAccount = TopDomainAccount::deserialize (&mut &**top_domain_account.data.borrow_mut()).unwrap_or_default();
        if top_domain_data_obj.account_state == AccountState::Initialized {
            msg!("domain account {} already exist", domain_name);
            return Err(ProgramError::AccountAlreadyInitialized)
        }

        if !Self::pubkeys_equal(multi_sig_account.key, &multi_sig_account_inline::id()) {
            msg!("invalid multi sig account.");
            return Err(NameError::InvalidMultiSigAccount.into())
        }
        let proposal = format!("{} init a create top domain {} proposal", payer_account.key, domain_name);
        // No need for check proposal account anymore, cause proposal account not reuse anymore, and
        // multi sig program provide the proposal account correctly
        // let (proposal_account_puk, _) = find_proposal_account(&multi_sig_account_inline::id(), 0);
        //
        // if !Self::pubkeys_equal(proposal_account.key, &proposal_account_puk) {
        //     msg!("invalid proposal account");
        //     return Err(NameError::InvalidProposalAccount.into())
        // }

        let proposal_data_obj: ProposalAccount = ProposalAccount::deserialize(&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();

        if proposal_data_obj.account_state == ppl_sig::state::AccountState::Uninitialized {
            msg!("start init proposal account");
            // create proposal
            invoke(
                &create_proposal_account(
                &ppl_sig::id(),
                proposal,
                PROPOSAL_EFFECT_PERIOD,
                &multi_sig_account.key,
                &payer_account.key,
                &proposal_account.key,

                )?,
                &[
                    multi_sig_account.clone(),
                    payer_account.clone(),
                    proposal_account.clone(),
                    system_program_account.clone(),
                    rent_account.clone(),
                    // multi_sig_program_account.clone()
                ]
            )?;
            return Ok(())
        }

        msg!("start verify proposal can execute");
        // verify proposal
        invoke(
           &verify(
            &ppl_sig::id(),
            &multi_sig_account.key,
            &payer_account.key,
            &proposal_account.key,
            Some(proposal)
           )?,
           &[
               multi_sig_account.clone(),
               payer_account.clone(),
               proposal_account.clone(),
           ],
        )?;

        // init top domain data
        top_domain_data_obj.account_type = AccountType::TopDomain;
        top_domain_data_obj.account_state = AccountState::Initialized;
        top_domain_data_obj.rule = rule;
        top_domain_data_obj.max_space = max_space;
        top_domain_data_obj.domain_name = domain_name;

        let top_domain_len = borsh::to_vec(&top_domain_data_obj).unwrap().len();
        Self::create_account(
            top_domain_len,
            &[
                top_domain_account.clone(),
                payer_account.clone(),
                rent_account.clone(),
                system_program_account.clone()
            ],
            program_id,
            plain_seeds
        )?;

        top_domain_data_obj.serialize(&mut *top_domain_account.data.borrow_mut())?;
        return Ok(())
    }

    /// Create Domain account.
    fn _create_domain(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, domain_name: String) -> ProgramResult{
        let domain_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let oracle_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        // Check the domain name format.
        if !is_valid_domain(&domain_name) {
            msg!("invalid domain name format");
            return Err(NameError::InvalidNameFormat.into())
        }
        let split_vec = domain_name.split(".").collect::<Vec<&str>>();
        if split_vec.len() != 2 {
            msg!("invalid domain name format, length unmatched");
            return Err(NameError::InvalidNameFormat.into())
        }
        let valid_domain_name = split_vec.get(0).unwrap();
        if valid_domain_name.len() <= 3 || valid_domain_name.len() > 32 {
            msg!("unsupported domain name len");
            return Err(NameError::InvalidNameFormat.into())
        }

        // Check the validity of the domain name
        let hash = hashv(&[domain_name.as_bytes()]);
        let (domain_pubkey, plain_seeds) = get_seeds_and_key(program_id,Some(hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();
        if !Self::pubkeys_equal(domain_account.key, &domain_pubkey) {
            msg!("unmatched domain name with domain account");
            return Err(ProgramError::InvalidArgument)
        }
        let mut domain_data_obj: DomainAccount= DomainAccount::deserialize (&mut &**domain_account.data.borrow_mut()).unwrap_or_default();
        // Account has been Initialized, and has not expired, not to create again.
        if domain_data_obj.account_state == AccountState::Initialized &&
            clock::Clock::get().unwrap().unix_timestamp < domain_data_obj.expire_time + GRACE_PERIOD {
            msg!("domain account {} already exist", domain_name);
            return Err(ProgramError::AccountAlreadyInitialized)
        }

        let top_account_data = parent_account.data.borrow();
        let top_domain_data_obj: TopDomainAccount = TopDomainAccount::deserialize (&mut &**top_account_data).unwrap();

        if top_domain_data_obj.account_state != AccountState::Initialized {
            msg!("invalid top domain");
            return Err(ProgramError::InvalidArgument)
        }

        // ask oracle for put -> usdt price
        let oracle_seeds = &["price".as_bytes(), &system_program::id().to_bytes(), &usdt_token_account::id().to_bytes()];
        let (oracle_account_puk, _) = Pubkey::find_program_address(oracle_seeds, &oracle_program::id());

        if *oracle_account.key != oracle_account_puk {
            msg!("invalid oracle account.");
            return Err(ProgramError::InvalidArgument)
        }
        let oracle_account_data_ref = &**oracle_account.data.borrow();
        let mut oracle_account_data = &oracle_account_data_ref[8..];
        let oracle_data_obj : PriceAccount = PriceAccount::deserialize (&mut oracle_account_data).unwrap();
        let usdt_to_put_price = Self::token_to_put(oracle_data_obj.price as u128);
        // compute fee
        let fee: u128;
        let fee_index: u32;
        match valid_domain_name.len() {
            4 => {
                fee_index = 1;
                fee = top_domain_data_obj.rule[1] * usdt_to_put_price
            },
            5 => {
                fee_index = 2;
                fee = top_domain_data_obj.rule[2] * usdt_to_put_price
            },
            6 => {
                fee_index = 3;
                fee = top_domain_data_obj.rule[3] * usdt_to_put_price
            },
            _ => {
                fee_index = 4;
                fee = top_domain_data_obj.rule[4] * usdt_to_put_price
            }
        }
        msg!("fee {}", fee);
        invoke(
            &system_instruction::transfer(payer_account.key, parent_account.key, fee),
            &[
                payer_account.clone(),
                parent_account.clone(),
                system_program_account.clone(),
            ],
        )?;

        let old_state = domain_data_obj.account_state;

        //init domain data
        domain_data_obj.account_type = AccountType::Domain;
        domain_data_obj.account_state = AccountState::Initialized;
        domain_data_obj.parent_key = *parent_account.key;
        domain_data_obj.owner = *owner_account.key;
        domain_data_obj.expire_time = clock::Clock::get().unwrap().unix_timestamp + EXPIRE_PERIOD;
        domain_data_obj.max_space = top_domain_data_obj.max_space;
        domain_data_obj.fee_index = fee_index;
        domain_data_obj.domain_name = domain_name;

        let domain_data_len = borsh::to_vec(&domain_data_obj).unwrap().len();

        if old_state == AccountState::Uninitialized {
            Self::create_account(
                domain_data_len,
                &[
                    domain_account.clone(),
                    payer_account.clone(),
                    rent_account.clone(),
                    system_program_account.clone(),
                ],
                program_id,
                plain_seeds
            )?
        } else {
           // condition created but expired
           // domain_name data length won't change after initiating,
           // its enough to pay to top domain receipt.
           invoke(
                &system_instruction::transfer(payer_account.key, parent_account.key, domain_account.lamports()),
                &[
                    payer_account.clone(),
                    parent_account.clone(),
                    system_program_account.clone(),
                ],
           )?;
        }
        domain_data_obj.serialize(&mut *domain_account.data.borrow_mut())?;

        return Ok({})
    }

    /// create rare domain, a multi sig interface.
    fn _create_rare_domain(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, domain_name: String) -> ProgramResult{
        let domain_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let multi_sig_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;
        let _multi_sig_program_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        // valid format
        if !is_valid_domain(&domain_name) {
            msg!("invalid domain name format");
            return Err(NameError::InvalidNameFormat.into())
        }
        let split_vec = domain_name.split(".").collect::<Vec<&str>>();
        if split_vec.len() != 2 {
            msg!("invalid domain name format");
            return Err(NameError::InvalidNameFormat.into())
        }
        let valid_domain_name = split_vec.get(0).unwrap();
        if valid_domain_name.len() > 3 {
            msg!("its not rare domain");
            return Err(NameError::InvalidNameFormat.into())
        }

        if !Self::pubkeys_equal(multi_sig_account.key, &multi_sig_account_inline::id()) {
            msg!("invalid multi sig account.");
            return Err(NameError::InvalidMultiSigAccount.into())
        }

        // Verify the validity of the domain name.
        let hash = hashv(&[domain_name.as_bytes()]);
        let (domain_pubkey, plain_seeds) = get_seeds_and_key(program_id,Some(hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();
        if !Self::pubkeys_equal(domain_account.key, &domain_pubkey)  {
            msg!("unmatched domain name with domain account");
            return Err(ProgramError::InvalidArgument)
        }
        let mut domain_data_obj: DomainAccount = DomainAccount::deserialize (&mut &**domain_account.data.borrow_mut()).unwrap_or_default();
        // Account has been Initialized, but has not expired, not to create again.
        if domain_data_obj.account_state == AccountState::Initialized &&
            clock::Clock::get().unwrap().unix_timestamp < domain_data_obj.expire_time + GRACE_PERIOD {
            msg!("domain account {} already exist", domain_name);
            return Err(ProgramError::AccountAlreadyInitialized)
        }

        // Check whether payment account match
        let top_domain_data_obj : TopDomainAccount = TopDomainAccount::deserialize (&mut &**parent_account.data.borrow_mut()).unwrap_or_default();

        if top_domain_data_obj.account_state != AccountState::Initialized {
            msg!("invalid top domain");
            return Err(ProgramError::InvalidArgument)
        }

        let proposal = format!("{} init a create rare domain {} proposal", payer_account.key, domain_name);

        let proposal_data_obj: ProposalAccount = ProposalAccount::deserialize(&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();

        if proposal_data_obj.account_state == ppl_sig::state::AccountState::Uninitialized {
            msg!("start init proposal account");
            // start to create proposal
            invoke(
                &create_proposal_account(
                    &ppl_sig::id(),
                    proposal,
                    PROPOSAL_EFFECT_PERIOD,
                    &multi_sig_account.key,
                    &payer_account.key,
                    &proposal_account.key,
                )?,
                &[
                    multi_sig_account.clone(),
                    payer_account.clone(),
                    proposal_account.clone(),
                    system_program_account.clone(),
                    rent_account.clone(),
                    // multi_sig_program_account.clone()
                ]
            )?;
            return Ok(())
        }

        msg!("start verify proposal can execute");
        // verify the proposal pass yet
        invoke(
            &verify(
                &ppl_sig::id(),
                &multi_sig_account.key,
                &payer_account.key,
                &proposal_account.key,
                Some(proposal)
            )?,
            &[
                multi_sig_account.clone(),
                payer_account.clone(),
                proposal_account.clone(),
            ],
        )?;
        let top_domain_account_data = parent_account.data.borrow();
        let top_domain_data_obj : TopDomainAccount = TopDomainAccount::deserialize (&mut &**top_domain_account_data).unwrap();
        let old_state = domain_data_obj.account_state;

        //init domain data
        domain_data_obj.account_type = AccountType::Domain;
        domain_data_obj.account_state = AccountState::Initialized;
        domain_data_obj.parent_key = *parent_account.key;
        domain_data_obj.owner = *owner_account.key;
        domain_data_obj.expire_time = clock::Clock::get().unwrap().unix_timestamp + EXPIRE_PERIOD;
        domain_data_obj.max_space = top_domain_data_obj.max_space;
        domain_data_obj.fee_index = 0;
        domain_data_obj.domain_name = domain_name;

        let domain_data_len = borsh::to_vec(&domain_data_obj).unwrap().len();
        if old_state == AccountState::Uninitialized {
            Self::create_account(
                domain_data_len,
                &[
                    domain_account.clone(),
                    payer_account.clone(),
                    rent_account.clone(),
                    system_program_account.clone()
                ],
                program_id,
                plain_seeds
            )?
        } else {
            // condition: Account has been created and expired
            // To give the parent costs into account payment account
            invoke(
                &system_instruction::transfer(payer_account.key, parent_account.key, domain_account.lamports()),
                &[
                    payer_account.clone(),
                    parent_account.clone(),
                    system_program_account.clone(),
                ],
            )?;
        }

        domain_data_obj.serialize(&mut *domain_account.data.borrow_mut())?;
        return Ok(())
    }

    /// Create domain resolve account.
    fn _create_domain_resolve_account(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, domain_name: String, value: Vec<u8>) -> ProgramResult{
        let domain_resolve_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        // let address_account = next_account_info(accounts_iter)?;
        let address_resolve_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        // Domain control account validity check.
        let domain_data_obj: DomainAccount = DomainAccount::deserialize (&mut &**parent_account.data.borrow_mut()).unwrap_or_default();
        if *owner_account.key != domain_data_obj.owner {
            msg!("invalid authority");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        if domain_data_obj.account_state != AccountState::Initialized {
            msg!("parent domain uninitialized");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        if  clock::Clock::get().unwrap().unix_timestamp as i64 > domain_data_obj.expire_time {
            msg!("parent domain expired");
            return Err(NameError::DomainExpired.into())
        }

        if !is_valid_domain(&domain_name) {
            msg!("invalid domain name format");
            return Err(NameError::InvalidNameFormat.into())
        }

        let split_vec = domain_name.split(".").collect::<Vec<&str>>();
        // Domain Level，vec len 2 -> level 1 resolve domain
        // 3 -> level 2 resolve domain， 4 -> level 3 resolve domain
        let domain_level = split_vec.len();
        if domain_level < 2 || domain_level > 4 {
            msg!("invalid domain name format");
            return Err(NameError::InvalidNameFormat.into())
        }
        // Take out the top-level domain.
        let top_domain = split_vec.get(domain_level - 1).unwrap();
        let domain = split_vec.get(domain_level - 2).unwrap();
        let complete_domain = domain.to_string() + "." + top_domain;

        // Verify the validity of the domain name
        let domain_hash = hashv(&[complete_domain.as_bytes()]);
        let (domain_pubkey,_) = get_seeds_and_key(
            program_id,
            Some(domain_hash.to_bytes().to_vec()),
            AccountType::Domain,
            None).unwrap();
        if domain_pubkey != *parent_account.key {
            msg!("unmatched domain name with parent domain");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        let hash = hashv(&[domain_name.as_bytes()]);
        let (resolve_domain_pubkey,domain_resolve_seeds) = get_seeds_and_key(
            program_id,
            Some(hash.to_bytes().to_vec()),
            AccountType::DomainResolve,
            None).unwrap();
        if !Self::pubkeys_equal(domain_resolve_account.key, &resolve_domain_pubkey) {
            msg!("unmatched resolve domain name with resolve domain account");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        // To check whether resolve domain name has been initialized
        let mut domain_resolve_data_obj: DomainResolveAccount =
            DomainResolveAccount::deserialize (&mut &**domain_resolve_account.data.borrow_mut()).unwrap_or_default();
        if domain_resolve_data_obj.account_state == AccountState::Initialized {
            msg!("resolve domain account {} already exist", domain_name);
            return Err(ProgramError::AccountAlreadyInitialized)
        }

        if value.len() > domain_data_obj.max_space as usize {
            msg!("Invalid value length.");
            return Err(NameError::InvalidValueLen.into())
        }

        domain_resolve_data_obj.parent_key = parent_account.key.clone();
        domain_resolve_data_obj.value = Some(value.clone());
        domain_resolve_data_obj.account_state = AccountState::Initialized;
        domain_resolve_data_obj.account_type = AccountType::DomainResolve;
        domain_resolve_data_obj.domain_name = domain_name.clone();
        let domain_resolve_data_len = borsh::to_vec(&domain_resolve_data_obj).unwrap().len();

        // todo Because there are some token, NFT some PDA account,
        // todo these accounts have no signature, so can not add a signature。 2022/10/28
        let (address_resolve_pubkey,address_resolve_seeds) =
            get_seeds_and_key(
                program_id,
                None,
                AccountType::AddressResolve,
                Some(value)).unwrap();

        if !Self::pubkeys_equal(address_resolve_account.key, &address_resolve_pubkey) {
            msg!("invalid address");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }
        let address_resolve_data_obj =
            AddressResolveAccount {
                account_type: AccountType::AddressResolve,
                account_state: AccountState::Initialized,
                domain: domain_name
        };
        let address_resolve_data_len = borsh::to_vec(&address_resolve_data_obj).unwrap().len();


        // Create a domain resolve account
        Self::create_account(
            domain_resolve_data_len,
            &[
                domain_resolve_account.clone(),
                payer_account.clone(),
                rent_account.clone(),
                system_program_account.clone(),
            ],
            program_id,
            domain_resolve_seeds
        )?;

        // Create address resolution account.
        Self::create_account(
            address_resolve_data_len,
            &[
                address_resolve_account.clone(),
                payer_account.clone(),
                rent_account.clone(),
                system_program_account.clone(),
            ],
            program_id,
            address_resolve_seeds
        )?;

        domain_resolve_data_obj.serialize(&mut *domain_resolve_account.data.borrow_mut())?;
        address_resolve_data_obj.serialize(&mut *address_resolve_account.data.borrow_mut())?;

        return ProgramResult::Ok({})
    }

    /// transfer domain
    fn _transfer(_: &Pubkey, accounts_iter: &mut Iter<AccountInfo>) -> ProgramResult{
        let domain_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let receipt_account = next_account_info(accounts_iter)?;

        // Account initialization check.
        let mut domain_data_obj : DomainAccount = DomainAccount::deserialize (&mut &**domain_account.data.borrow_mut()).unwrap_or_default();
        if domain_data_obj.account_state != AccountState::Initialized {
            msg!("domain account {} not exist", domain_account.key);
            return Err(NameError::AccountNotExist.into())
        }
        if domain_data_obj.owner != *owner_account.key {
            msg!("owner mismatched");
            return  Err(ProgramError::IllegalOwner)
        }

        domain_data_obj.owner = *receipt_account.key;

        domain_data_obj.serialize(&mut *domain_account.data.borrow_mut())?;

        return Ok(())
    }

    fn _update_domain_resolve_value(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, new_value: Vec<u8>, domain_name: String) -> ProgramResult{
        let domain_resolve_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let old_address_resolve_account = next_account_info(accounts_iter)?;
        let new_address_resolve_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let top_domain_account = next_account_info(accounts_iter)?;

        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        if *old_address_resolve_account.key == *new_address_resolve_account.key {
            msg!("value unchanged");
            return Err(ProgramError::InvalidArgument)
        }

        // To valid whether domain owner
        let domain_data_obj : DomainAccount = DomainAccount::deserialize (&mut &**parent_account.data.borrow_mut()).unwrap_or_default();
        if *owner_account.key != domain_data_obj.owner {
            msg!("invalid authority");
            return Err(ProgramError::IllegalOwner)
        }

        // To check whether parse domain name has been initialized
        let mut domain_resolve_data_obj: DomainResolveAccount =
            DomainResolveAccount::deserialize (&mut &**domain_resolve_account.data.borrow_mut()).unwrap_or_default();
        if domain_resolve_data_obj.account_state != AccountState::Initialized {
            msg!("resolve domain account {} not exist", domain_resolve_account.key);
            return Err(ProgramError::InvalidArgument)
        }

        // if the domain resolve account not unbind, delete them
        if domain_resolve_data_obj.value.is_some() {
            // let old_address_resolve_data = old_address_resolve_account.data.borrow();
            let top_domain_account_lamports = top_domain_account.lamports();
            **top_domain_account.lamports.borrow_mut() = top_domain_account_lamports
                .checked_add(old_address_resolve_account.lamports())
                .ok_or(ProgramError::InvalidArgument)?;
            // Delete the old account
            **old_address_resolve_account.lamports.borrow_mut() = 0;
            let data_len = old_address_resolve_account.data_len();
            put_memset(*old_address_resolve_account.data.borrow_mut(), 0, data_len);
        }

        if new_value.len() > domain_data_obj.max_space as usize {
            msg!("Invalid value length.");
            return Err(NameError::InvalidValueLen.into())
        }

        domain_resolve_data_obj.value = Some(new_value.clone());

        let (new_address_resolve_pubkey,plain_seeds) = get_seeds_and_key(program_id, None, AccountType::AddressResolve, Some(new_value)).unwrap();
        if !Self::pubkeys_equal(new_address_resolve_account.key, &new_address_resolve_pubkey) {
            msg!("invalid address");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }
        let new_address_resolve_data_obj = AddressResolveAccount{ account_type: AccountType::AddressResolve, account_state: AccountState::Initialized, domain: domain_name };
        let address_resolve_data = borsh::to_vec(&new_address_resolve_data_obj).unwrap();

        // Create a new address resolution account.
        Self::create_account(
            address_resolve_data.len(),
            &[
                new_address_resolve_account.clone(),
                payer_account.clone(),
                rent_account.clone(),
                system_program_account.clone(),
            ],
            program_id,
            plain_seeds
        )?;
        domain_resolve_data_obj.serialize(&mut *domain_resolve_account.data.borrow_mut())?;
        new_address_resolve_data_obj.serialize(&mut *new_address_resolve_account.data.borrow_mut())?;

        return Ok({})
    }

    fn _unbind_address(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>) -> ProgramResult{
        let domain_resolve_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let address_resolve_account = next_account_info(accounts_iter)?;
        let top_domain_account = next_account_info(accounts_iter)?;

        // Check whether parse domain name has been initialized.
        let mut domain_resolve_data_obj: DomainResolveAccount =
            DomainResolveAccount::deserialize (&mut &**domain_resolve_account.data.borrow_mut()).unwrap_or_default();
        if domain_resolve_data_obj.account_state != AccountState::Initialized {
            msg!("resolve domain account {} not exist", domain_resolve_account.key);
            return Err(NameError::AccountNotExist.into())
        }

        // Verify whether repeat unbundling
        if domain_resolve_data_obj.value.is_none() {
            msg!("duplicated unbind");
            return Err(NameError::RepeatUnbind.into())
        }

        // Check domain account and the account owner is effective.
        if domain_resolve_data_obj.parent_key != *parent_account.key {
            msg!("unmatched domain and child domain");
            return ProgramResult::Err(ProgramError::IllegalOwner)
        }

        let domain_data_obj: DomainAccount = DomainAccount::deserialize (&mut &**parent_account.data.borrow_mut()).unwrap_or_default();
        if domain_data_obj.owner != *owner_account.key {
            msg!("unmatched owner and domain account");
            return ProgramResult::Err(ProgramError::IllegalOwner)
        }

        let (address_resolve_pubkey,_) = get_seeds_and_key(program_id, None, AccountType::AddressResolve, domain_resolve_data_obj.value).unwrap();
        if !Self::pubkeys_equal(address_resolve_account.key, &address_resolve_pubkey) {
            msg!("invalid address");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        // Delete the address resolution account.
        let top_domain_account_lamports = top_domain_account.lamports();
        **top_domain_account.lamports.borrow_mut() = top_domain_account_lamports
            .checked_add(address_resolve_account.lamports())
            .ok_or(ProgramError::InvalidArgument)?;

        **address_resolve_account.lamports.borrow_mut() = 0;
        let data_len = address_resolve_account.data_len();
        put_memset(*address_resolve_account.data.borrow_mut(), 0, data_len);

        domain_resolve_data_obj.value = None;
        domain_resolve_data_obj.serialize(&mut *domain_resolve_account.data.borrow_mut())?;

        return Ok({})
    }

    fn _close_domain_resolve_account(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>) -> ProgramResult{
        let domain_resolve_account = next_account_info(accounts_iter)?;
        let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let address_resolve_account = next_account_info(accounts_iter)?;
        let top_domain_account = next_account_info(accounts_iter)?;

        // To Check whether parse domain name has been initialized.
        let domain_resolve_data_obj: DomainResolveAccount =
            DomainResolveAccount::deserialize (&mut &**domain_resolve_account.data.borrow_mut()).unwrap_or_default();
        if domain_resolve_data_obj.account_state != AccountState::Initialized {
            msg!("resolve domain account {} not exist", domain_resolve_account.key);
            return ProgramResult::Err(ProgramError::UninitializedAccount)
        }

        // Check domain account and the account owner is effective.
        if domain_resolve_data_obj.parent_key != *parent_account.key {
            msg!("unmatched domain and child domain");
            return Err(ProgramError::InvalidArgument)
        }

        let domain_data_obj: DomainAccount = DomainAccount::deserialize (&mut &**parent_account.data.borrow_mut()).unwrap();
        if domain_data_obj.owner != *owner_account.key {
            msg!("unmatched owner and domain account");
            return Err(ProgramError::InvalidArgument)
        }
        let mut address_balance : u128 = 0;
        // If the domain resolve account is not bundling, delete the address resolution account.
        if domain_resolve_data_obj.value.is_some() {
            let (address_resolve_pubkey,_) = get_seeds_and_key(program_id, None, AccountType::AddressResolve, domain_resolve_data_obj.value).unwrap();
            if address_resolve_account.key != &address_resolve_pubkey {
                msg!("invalid address");
                return Err(ProgramError::InvalidArgument)
            }
            address_balance = address_resolve_account.lamports();
            let address_data_len = address_resolve_account.data_len();
            // Delete the address resolution account
            **address_resolve_account.lamports.borrow_mut() = 0;
            put_memset(*address_resolve_account.data.borrow_mut(), 0, address_data_len);
        }

        let top_domain_account_lamports = top_domain_account.lamports();
        **top_domain_account.lamports.borrow_mut() = top_domain_account_lamports
            .checked_add(address_balance + domain_resolve_account.lamports())
            .ok_or(ProgramError::InvalidArgument)?;


        let resolve_data_len = domain_resolve_account.data_len();
        **domain_resolve_account.lamports.borrow_mut() = 0;
        put_memset(*domain_resolve_account.data.borrow_mut(), 0, resolve_data_len);

        return Ok(())
    }

    /// renewal domain
    fn _renewal_domain(_: &Pubkey, accounts_iter: &mut Iter<AccountInfo>) -> ProgramResult{
        let domain_account = next_account_info(accounts_iter)?;
        // let owner_account = next_account_info(accounts_iter)?;
        let parent_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let oracle_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;

        let mut domain_data_obj: DomainAccount = DomainAccount::deserialize (&mut &**domain_account.data.borrow_mut()).unwrap_or_default();
        if domain_data_obj.account_state != AccountState::Initialized {
            msg!("domain account not Initialized");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        if domain_data_obj.parent_key != *parent_account.key {
            msg!("mismatched domain account and top domain account");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }

        // pay
        let top_domain_data_obj: TopDomainAccount= TopDomainAccount::deserialize (&mut &**parent_account.data.borrow_mut()).unwrap_or_default();

        // Tolerance period has passed unable to renewal
        if clock::Clock::get().unwrap().unix_timestamp > domain_data_obj.expire_time + GRACE_PERIOD {
            msg!("domain not exist(beyond grace period)");
            return ProgramResult::Err(ProgramError::UninitializedAccount)
        }
        // Timeout calculation starting point.
        let mut start_time = clock::Clock::get().unwrap().unix_timestamp;
        if clock::Clock::get().unwrap().unix_timestamp < domain_data_obj.expire_time {
            start_time = domain_data_obj.expire_time;
        }

        // Tolerance period fee
        let mut grace_period_fee : u128 = 0;
        if clock::Clock::get().unwrap().unix_timestamp > domain_data_obj.expire_time + GRACE_PERIOD {
            grace_period_fee = GRACE_PERIOD_FEE
        }

        // ask oracle for usdt -> put price
        let oracle_seeds = &["price".as_bytes(), &system_program::id().to_bytes(), &usdt_token_account::id().to_bytes()];
        let (oracle_account_puk, _) = Pubkey::find_program_address(oracle_seeds, &oracle_program::id());
        if *oracle_account.key != oracle_account_puk {
            msg!("invalid oracle account.");
            return ProgramResult::Err(ProgramError::InvalidArgument)
        }
        let oracle_account_data_ref = &**oracle_account.data.borrow();
        let mut oracle_account_data = &oracle_account_data_ref[8..];
        let oracle_data_obj : PriceAccount = PriceAccount::deserialize (&mut oracle_account_data).unwrap();
        let usdt_to_put_price = Self::token_to_put(oracle_data_obj.price as u128);
        // compute fee
        let fee = (top_domain_data_obj.rule[domain_data_obj.fee_index as usize] + grace_period_fee) * usdt_to_put_price as u128;

        if top_domain_data_obj.account_state != AccountState::Initialized {
            msg!("invalid top domain");
            return Err(ProgramError::InvalidArgument)
        }

        invoke(
            &system_instruction::transfer(payer_account.key, parent_account.key, fee),
            &[
                payer_account.clone(),
                parent_account.clone(),
                system_program_account.clone(),
            ],
        )?;
        // Initializes the domain data.
        domain_data_obj.expire_time = start_time + EXPIRE_PERIOD;
        domain_data_obj.serialize(&mut *domain_account.data.borrow_mut())?;

        return Ok(())
    }

    /// Set the top domain account's receipt account.
    fn _set_top_receipt(accounts_iter: &mut Iter<AccountInfo>, receipt_puk : Pubkey) -> ProgramResult{
        let top_domain_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let multi_sig_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;
        let _multi_sig_program_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        if !Self::pubkeys_equal(multi_sig_account.key, &multi_sig_account_inline::id()) {
            msg!("invalid multi sig account.");
            return Err(NameError::InvalidMultiSigAccount.into())
        }

        let top_domain_data_obj : TopDomainAccount = TopDomainAccount::deserialize (&mut &**top_domain_account.data.borrow_mut()).unwrap_or_default();
        if top_domain_data_obj.account_state != AccountState::Initialized {
            msg!("uninitialized account {}", top_domain_account.key);
            return Err(ProgramError::UninitializedAccount)
        }

        let proposal = format!("{} init a set top domain account {} receipt account to {} proposal", payer_account.key, top_domain_account.key, receipt_puk);

        let proposal_data_obj: ProposalAccount = ProposalAccount::deserialize(&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();

        if proposal_data_obj.account_state == ppl_sig::state::AccountState::Uninitialized {
            msg!("start init proposal account");
            // create proposal
            invoke(
                &create_proposal_account(
                    &ppl_sig::id(),
                    proposal,
                    PROPOSAL_EFFECT_PERIOD,
                    &multi_sig_account.key,
                    &payer_account.key,
                    &proposal_account.key,
                )?,
                &[
                    multi_sig_account.clone(),
                    payer_account.clone(),
                    proposal_account.clone(),
                    system_program_account.clone(),
                    rent_account.clone(),
                ]
            )?;
            return Ok(())
        }

        msg!("start verify proposal can execute");
        // verify proposal
        invoke(
            &verify(
                &ppl_sig::id(),
                &multi_sig_account.key,
                &payer_account.key,
                &proposal_account.key,
                Some(proposal)
            )?,
            &[
                multi_sig_account.clone(),
                payer_account.clone(),
                proposal_account.clone(),
            ],
        )?;

        top_domain_data_obj.serialize(&mut *top_domain_account.data.borrow_mut())?;

        return Ok(())
    }

    /// create a new account
    /// account_infos:
    ///     1、new_account;
    ///     2、payer_account;
    ///     3、rent_account;
    ///     4、system_account;
    pub fn create_account(
        data_len: usize,
        account_infos: &[AccountInfo],
        program_id: &Pubkey,
        plain_seeds: Vec<u8>
    ) -> ProgramResult {
        let mut account_iter = account_infos.into_iter();
        let new_account = next_account_info(&mut account_iter)?;
        let payer_account = next_account_info(&mut account_iter)?;
        let rent_account = next_account_info(&mut account_iter)?;
        let system_program_account = next_account_info(&mut account_iter)?;


        let rent = &Rent::from_account_info(rent_account)?;
        let required_lamports = rent
            .minimum_balance(data_len)
            .max(1)
            .saturating_sub(new_account.lamports());

        if required_lamports > 0 {
            msg!("Transfer {} lamports to the new account", required_lamports);
            invoke(
                &system_instruction::transfer(payer_account.key, new_account.key, required_lamports),
                &[
                    payer_account.clone(),
                    new_account.clone(),
                    system_program_account.clone(),
                ],
            )?;
        }

        // alloc account data space
        let allocate_accounts = &[new_account.clone(), system_program_account.clone()];
        msg!("Allocate space for the account");

        let mut seeds_chunk = plain_seeds.chunks(32).collect::<Vec<&[u8]>>();
        let (_, bump) =
            Pubkey::find_program_address(&seeds_chunk, &program_id);
        let bump_seed = [bump];
        seeds_chunk.push(&bump_seed);
        let signer_seeds = seeds_chunk.as_slice();

        invoke_signed(
            &system_instruction::allocate(new_account.key, data_len as u64),
            allocate_accounts,
            &[signer_seeds],
        )?;


        msg!("Assign the account to the owning program");
        invoke_signed(
            &system_instruction::assign(new_account.key, program_id),
            allocate_accounts,
            &[signer_seeds],
        )?;

        Ok(())
    }

    /// Checks two pubkeys for equality in a computationally cheap way using
    /// `sol_memcmp`
    pub fn pubkeys_equal(a: &Pubkey, b: &Pubkey) -> bool {
        put_memcmp(a.as_ref(), b.as_ref(), PUBKEY_BYTES) == 0
    }

    /// through 1 put -> target token price,
    /// get 1 target token -> put price,
    /// decimal is 9
    fn token_to_put(token_price: u128) -> u128 {
        let rate = PUT_BASE.div(token_price as f64);
        let ret = (PUT_BASE * rate) as u128;
        ret
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        msg!("start to process");
        let instruction = NameInstruction::deserialize(input).unwrap();
        let mut accounts_iter = accounts.iter();
        match instruction {

            NameInstruction::CreateTopDomain { domain_name, rule , max_space} => {
                msg!("start to process CreateTopDomain instruction");
                Self::_create_top_domain(program_id, &mut accounts_iter, domain_name, rule, max_space)
            }

            NameInstruction::CreateDomain {domain_name} => {
                msg!("start to process CreateDomain instruction");
                Self::_create_domain(program_id, &mut accounts_iter, domain_name)
            }

            NameInstruction::CreateRareDomain {domain_name} => {
                msg!("start to process CreateDomain instruction");
                Self::_create_rare_domain(program_id, &mut accounts_iter, domain_name)
            }

            NameInstruction::CreateDomainResolveAccount {domain_name, value} => {
                msg!("start to process create_domain_resolve_account instruction");
                Self::_create_domain_resolve_account(program_id, &mut accounts_iter, domain_name, value)
            }

            NameInstruction::Transfer => {
                msg!("start to process transfer instruction");
                Self::_transfer(program_id, &mut accounts_iter)
            }

            NameInstruction::UpdateDomainResolveAccount{new_value, domain_name} => {
                msg!("start to process update_domain_resolve_account instruction");
                Self::_update_domain_resolve_value(program_id, &mut accounts_iter, new_value, domain_name)
            }

            NameInstruction::CloseDomainResolveAccount => {
                msg!("start to process close_domain_resolve_account instruction");
                Self::_close_domain_resolve_account(program_id, &mut accounts_iter)
            }

            NameInstruction::UnbindAddressAccount => {
                msg!("start to process unbind_address_account instruction");
                Self::_unbind_address(program_id, &mut accounts_iter)
            }

            NameInstruction::Renewal => {
                msg!("start to process unbind_address_account instruction");
                Self::_renewal_domain(program_id, &mut accounts_iter)
            }

            NameInstruction::SetTopReceipt { new_receipt_account} => {
                msg!("start to process set_top_receipt instruction");
                Self::_set_top_receipt(&mut accounts_iter, new_receipt_account)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::{Div};
    use put_program::entrypoint::ProgramResult;
    use put_program::instruction::Instruction;
    use put_program::pubkey::Pubkey;
    use put_sdk::account::{Account as PUTAccount, create_is_signer_account_infos};
    use crate::error::NameError;
    use crate::instruction::{create_transfer, unbind_address_account};
    use crate::processor::Processor;
    use crate::state::{AccountState, AccountType, DomainAccount, DomainResolveAccount, get_seeds_and_key, TopDomainAccount};
    use borsh::BorshDeserialize;
    use put_program::program_error::ProgramError;

    #[test]
    fn rate_compute() {
        let base : f64 = 100000000_f64;
        let rate = base.div(10000000 as f64);
        println!("rate {}", rate);
        let base = (base * rate) as u128;
        println!("base {}", base);
    }

    fn do_process_instruction(
        instruction: Instruction,
        accounts: Vec<&mut PUTAccount>,
    ) -> ProgramResult {
        let mut meta = instruction
            .accounts
            .iter()
            .zip(accounts)
            .map(|(account_meta, account)| (&account_meta.pubkey, account_meta.is_signer, account))
            .collect::<Vec<_>>();

        let account_infos = create_is_signer_account_infos(&mut meta);
        Processor::process(&instruction.program_id, &account_infos, &instruction.data)
    }

    #[test]
    fn test_puk_equal() {
        let puk1 = Pubkey::new_unique();
        println!("puk1 {}", puk1);
        let puk2 = Pubkey::new_unique();
        println!("puk2 {}", puk2);
        assert_eq!(false, puk1 == puk2);
        let puk1_clone = puk1.clone();
        assert_eq!(true, puk1 == puk1_clone)
    }

    #[test]
    fn test_transfer() {
        let program_id = crate::id();
        let domain_account_puk = Pubkey::new_unique();
        let domain_owner_account_puk = Pubkey::new_unique();
        let receipt_account_puk = Pubkey::new_unique();

        let mut domain_account_data_obj = DomainAccount{
            account_type: Default::default(),
            account_state: Default::default(),
            parent_key: Default::default(),
            owner: Default::default(),
            expire_time: 0,
            max_space: 32,
            fee_index: 0,
            domain_name: "fb.put".to_string()
        };

        let domain_account_data = borsh::to_vec(&domain_account_data_obj).unwrap();
        let mut domain_account = PUTAccount::new(1000, domain_account_data.len(), &program_id);
        domain_account.data = domain_account_data;
        // let mut rent_sysvar = rent_sysvar();
        let mut domain_owner_account = PUTAccount::default();
        let mut receipt_account = PUTAccount::default();

        // AccountNotExist
        assert_eq!(
            Err(NameError::AccountNotExist.into()),
            do_process_instruction(
                create_transfer(program_id, domain_account_puk, domain_owner_account_puk, receipt_account_puk).unwrap(),
                vec![&mut domain_account, &mut domain_owner_account, &mut receipt_account]
            )
        );

        domain_account_data_obj.account_state = AccountState::Initialized;
        let domain_account_data = borsh::to_vec(&domain_account_data_obj).unwrap();
        domain_account.data = domain_account_data;

        // IllegalOwner
        assert_eq!(
            Err(ProgramError::IllegalOwner),
            do_process_instruction(
                create_transfer(program_id, domain_account_puk, domain_owner_account_puk, receipt_account_puk).unwrap(),
                vec![&mut domain_account, &mut domain_owner_account, &mut receipt_account]
            )
        );

        domain_account_data_obj.owner = domain_owner_account_puk;
        let domain_account_data = borsh::to_vec(&domain_account_data_obj).unwrap();
        domain_account.data = domain_account_data;
        let domain_account_before_transfer : DomainAccount = DomainAccount::deserialize(&mut domain_account.data.as_slice()).unwrap();
        assert_ne!(
            receipt_account_puk,
            domain_account_before_transfer.owner
        );
        println!("old owner {}", domain_account_before_transfer.owner);
        println!("new owner {}", receipt_account_puk);
        do_process_instruction(
            create_transfer(program_id, domain_account_puk, domain_owner_account_puk, receipt_account_puk).unwrap(),
            vec![&mut domain_account, &mut domain_owner_account, &mut receipt_account]
        );
        let domain_account_after_transfer : DomainAccount = DomainAccount::deserialize(&mut domain_account.data.as_slice()).unwrap();
        assert_eq!(
            receipt_account_puk,
            domain_account_after_transfer.owner
        );
        //owner changed
        println!("now owner {}", domain_account_after_transfer.owner);
    }

    #[test]
    fn test_unbind_address() {

        let program_id = crate::id();
        let domain_resolve_account_puk = Pubkey::new_unique();
        let owner_account_puk = Pubkey::new_unique();
        let parent_account_puk = Pubkey::new_unique();
        let address_resolve_account_puk = Pubkey::new_unique();
        let top_domain_account_puk = Pubkey::new_unique();

        let old_address_value = Pubkey::new_unique();

        let mut domain_resolve_account_obj = DomainResolveAccount{
            account_type: Default::default(),
            account_state: Default::default(),
            parent_key: Default::default(),
            domain_name: "".to_string(),
            value: None
        };

        let domain_resolve_account_data = borsh::to_vec(&domain_resolve_account_obj).unwrap();
        let mut domain_resolve_account = PUTAccount::new(1000, domain_resolve_account_data.len(), &program_id);
        domain_resolve_account.data = domain_resolve_account_data;
        // let mut rent_sysvar = rent_sysvar();
        let mut owner_account = PUTAccount::default();
        let mut parent_account_data_obj = DomainAccount{
            account_type: Default::default(),
            account_state: Default::default(),
            parent_key: Default::default(),
            owner: Default::default(),
            expire_time: 0,
            max_space: 32,
            fee_index: 0,
            domain_name: "fb.put".to_string()
        };
        let parent_account_data = borsh::to_vec(&parent_account_data_obj).unwrap();
        let mut parent_account = PUTAccount::new(1000, parent_account_data.len(), &program_id);
        parent_account.data = parent_account_data;
        let mut address_resolve_account = PUTAccount::new(1000, 43, &program_id);

        let mut top_domain_account_obj = TopDomainAccount{
            account_type: Default::default(),
            account_state: Default::default(),
            rule: [0, 0, 0, 0 ,0],
            max_space: 0,
            domain_name: "".to_string()
        };

        let top_domain_account_data = borsh::to_vec(&top_domain_account_obj).unwrap();
        let mut top_domain_account = PUTAccount::new(1000, top_domain_account_data.len(), &program_id);
        top_domain_account.data = top_domain_account_data;


        // AccountNotExist
        assert_eq!(
            Err(NameError::AccountNotExist.into()),
            do_process_instruction(
                unbind_address_account(
                    program_id,
                    domain_resolve_account_puk,
                    owner_account_puk,
                    parent_account_puk,
                    address_resolve_account_puk,
                    top_domain_account_puk,
                ).unwrap(),
                vec![
                    &mut domain_resolve_account,
                    &mut owner_account,
                    &mut parent_account,
                    &mut address_resolve_account,
                    &mut top_domain_account,
                ]
            )
        );

        // set account state.
        domain_resolve_account_obj.account_state = AccountState::Initialized;
        let domain_resolve_account_data = borsh::to_vec(&domain_resolve_account_obj).unwrap();
        domain_resolve_account.data = domain_resolve_account_data;

        // RepeatUnbind
        assert_eq!(
            Err(NameError::RepeatUnbind.into()),
            do_process_instruction(
                unbind_address_account(
                    program_id,
                    domain_resolve_account_puk,
                    owner_account_puk,
                    parent_account_puk,
                    address_resolve_account_puk,
                    top_domain_account_puk,
                ).unwrap(),
                vec![
                    &mut domain_resolve_account,
                    &mut owner_account,
                    &mut parent_account,
                    &mut address_resolve_account,
                    &mut top_domain_account,
                ]
            )
        );

        // set invalid value
        domain_resolve_account_obj.value = Some(old_address_value.to_bytes().into());
        let domain_resolve_account_data = borsh::to_vec(&domain_resolve_account_obj).unwrap();
        domain_resolve_account.data = domain_resolve_account_data;

        assert_eq!(
            Err(ProgramError::IllegalOwner.into()),
            do_process_instruction(
                unbind_address_account(
                    program_id,
                    domain_resolve_account_puk,
                    owner_account_puk,
                    parent_account_puk,
                    address_resolve_account_puk,
                    top_domain_account_puk,
                ).unwrap(),
                vec![
                    &mut domain_resolve_account,
                    &mut owner_account,
                    &mut parent_account,
                    &mut address_resolve_account,
                    &mut top_domain_account,
                ]
            )
        );

        // set parent account.
        domain_resolve_account_obj.parent_key = parent_account_puk;
        let domain_resolve_account_data = borsh::to_vec(&domain_resolve_account_obj).unwrap();
        domain_resolve_account.data = domain_resolve_account_data;

        // set domain(parent) account owner.
        parent_account_data_obj.owner = owner_account_puk;
        let parent_account_data = borsh::to_vec(&parent_account_data_obj).unwrap();
        parent_account.data = parent_account_data;

        assert_eq!(
            Err(ProgramError::InvalidArgument.into()),
            do_process_instruction(
                unbind_address_account(
                    program_id,
                    domain_resolve_account_puk,
                    owner_account_puk,
                    parent_account_puk,
                    address_resolve_account_puk,
                    top_domain_account_puk,
                ).unwrap(),
                vec![
                    &mut domain_resolve_account,
                    &mut owner_account,
                    &mut parent_account,
                    &mut address_resolve_account,
                    &mut top_domain_account,
                ]
            )
        );

        // set valid address account
        let (real_address_resolve_pubkey,_) = get_seeds_and_key(
            &program_id,
            None,
            AccountType::AddressResolve,
            Some(old_address_value.to_bytes().into())
        ).unwrap();

        let top_domain_account_data = borsh::to_vec(&top_domain_account_obj).unwrap();
        top_domain_account.data = top_domain_account_data;

        // execute program success
        do_process_instruction(
            unbind_address_account(
                program_id,
                domain_resolve_account_puk,
                owner_account_puk,
                parent_account_puk,
                real_address_resolve_pubkey,
                top_domain_account_puk,
            ).unwrap(),
            vec![
                &mut domain_resolve_account,
                &mut owner_account,
                &mut parent_account,
                &mut address_resolve_account,
                &mut top_domain_account,
            ]
        );
        // Valid whether changed value is expected
        let domain_resolve_account_changed_obj : DomainResolveAccount = DomainResolveAccount::deserialize(&mut domain_resolve_account.data.as_slice()).unwrap();

        assert_eq!(
            domain_resolve_account_changed_obj.value,
            None
        )

    }
}