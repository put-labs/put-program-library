use std::collections::{HashSet};
use std::ops::Add;
use std::slice::Iter;
use put_program::account_info::{AccountInfo, next_account_info};
use put_program::entrypoint::ProgramResult;
use put_program::{clock, msg, system_instruction};
use put_program::program_error::ProgramError;
use put_program::pubkey::{Pubkey};
use crate::instruction::{SigInstruction};
use crate::state::{AccountState, MAX_MULTI_SIG_ACCOUNTS, MultiSigAccount, PROPOSAL_EFFECT_PERIOD, ProposalAccount, pubkeys_equal};
use borsh::{BorshDeserialize, BorshSerialize};
use put_program::program::{invoke, invoke_signed};
use put_program::program_memory::{put_memset};
use put_program::rent::Rent;
use put_program::sysvar::Sysvar;
use crate::error::SigError;

/// Program state handler.
pub struct Processor {}

impl Processor {

    /// Create multi sig account.
    fn _create_multi_sig_account(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, mut accounts: Vec<Pubkey>, threshold: u8) -> ProgramResult{
        let multi_sig_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        // Check the threshold effectiveness.
        if threshold < 1 || threshold > 100 {
            msg!("invalid threshold value");
            return Err(SigError::InvalidThreshold.into())
        }

        if accounts.len() > MAX_MULTI_SIG_ACCOUNTS {
            msg!("Too many accounts");
            return Err(SigError::TooManyAccounts.into())
        }

        if accounts.len() == 0 {
            msg!("Singers must greater than 0");
            return Err(SigError::SignersNotBeEmpty.into())
        }

        let mut filter_accounts = HashSet::new();
        accounts
            .iter()
            .for_each(|x| {
                filter_accounts.insert(x);
            });

        if filter_accounts.len() != accounts.len() {
            msg!("signer accounts not allow repeat");
            return Err(SigError::SingerCanNotRepeat.into())
        }

        let mut sig_accounts = [(Pubkey::default(), false); MAX_MULTI_SIG_ACCOUNTS];

        for (index, account) in accounts.iter_mut().enumerate() {
            sig_accounts[index] = (*account, true);
        }

        let sig_data_obj = MultiSigAccount{
            account_state: AccountState::Initialized,
            accounts: sig_accounts,
            threshold,
            nonce: 0
        };


        let sig_data_len = borsh::to_vec(&sig_data_obj).unwrap().len();

        let mut seeds_vec: Vec<u8> = vec![];
        seeds_vec.append(&mut payer_account.key.to_bytes().into());
        seeds_vec.append(&mut multi_sig_account.key.to_bytes().into());
        seeds_vec.append(&mut program_id.to_bytes().into());

        Self::create_pda_account_with_seeds(
            sig_data_len,
            &[
                multi_sig_account.clone(),
                payer_account.clone(),
                rent_account.clone(),
                system_program_account.clone(),
            ],
            program_id,
            seeds_vec
        )?;


        sig_data_obj.serialize(&mut *multi_sig_account.data.borrow_mut())?;

        return Ok(())
    }

    /// Init the proposal account.
    fn _init_proposal_account(
        program_id: &Pubkey,
        accounts_iter: &mut Iter<AccountInfo>,
        parent_key: Pubkey,
        summary: String,
        continuous_validity_period: u32
    ) -> ProgramResult{
        let multi_sig_account = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;
        let rent_account = next_account_info(accounts_iter)?;

        let mut multi_sig_data_obj : MultiSigAccount =
            MultiSigAccount::deserialize (&mut &**multi_sig_account.data.borrow_mut()).unwrap_or_default();
        if multi_sig_data_obj.account_state != AccountState::Initialized {
            msg!("multi sig account uninitialized");
            return Err(SigError::AccountUnInitialized.into())
        }

        if !multi_sig_data_obj.is_signer(payer_account.key) {
            msg!("only signer can create proposal.");
            return Err(SigError::NoAuthority.into())
        }
        // proposal account address seeds build by
        // multi_sig_account address and latest nonce
        let mut seeds_vec: Vec<u8> = vec![];
        seeds_vec.append(&mut multi_sig_account.key.clone().to_bytes().into());
        let new_nonce = multi_sig_data_obj.nonce + 1;
        seeds_vec.append(&mut new_nonce.to_le_bytes().into());

        let (proposal_account_puk, _) =
            Pubkey::find_program_address(&seeds_vec.chunks(32).collect::<Vec<&[u8]>>(), program_id);

        // in concurrent, nonce get from rpc maybe not latest,
        // we know and allow this case happen, and just return an error
        // for avoiding nonce reuse.
        if !pubkeys_equal(&proposal_account_puk, proposal_account.key) {
            msg!("Unmatched summary with proposal account.");
            return Err(SigError::InvalidProposalAccount.into())
        }

        let proposal_data_obj : ProposalAccount =
            ProposalAccount::deserialize (&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();
        // remove proposal account expire time check, cause proposal account not reuse anymore.
        if proposal_data_obj.account_state == AccountState::Initialized {
            msg!("proposal account already exist.");
            return Err(SigError::AccountAlreadyExist.into())
        }

        let mut vote_state = [(Pubkey::default(), false); MAX_MULTI_SIG_ACCOUNTS];

        vote_state[0] = (payer_account.key.clone(), true);

        let summary = summary.add(format!(", nonce {}", multi_sig_data_obj.nonce + 1).as_str());
        // Note: change nonce in this place rather than clients, for avoiding dirty data
        let proposal_data_obj = ProposalAccount {
            account_state: AccountState::Initialized,
            tickets: vote_state,
            parent: parent_key,
            tx_expired_time: clock::Clock::get().unwrap().unix_timestamp + continuous_validity_period as i64,
            initiator: payer_account.key.clone(),
            summary,
            nonce: multi_sig_data_obj.nonce + 1
        };

        let proposal_data_len = borsh::to_vec(&proposal_data_obj).unwrap().len();

        Self::create_pda_account_with_seeds(
            proposal_data_len,
            &[
                proposal_account.clone(),
                payer_account.clone(),
                rent_account.clone(),
                system_program_account.clone(),
            ],
            program_id,
            seeds_vec
        )?;

        multi_sig_data_obj.nonce += 1;

        proposal_data_obj.serialize(&mut *proposal_account.data.borrow_mut())?;
        multi_sig_data_obj.serialize(&mut *multi_sig_account.data.borrow_mut())?;

        return Ok(())
    }

    /// Vote for proposal.
    fn _vote(_: &Pubkey, accounts_iter: &mut Iter<AccountInfo>) -> ProgramResult{
        let multi_sig_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;
        let signer_account = next_account_info(accounts_iter)?;


        let multi_sig_data_obj : MultiSigAccount =
            MultiSigAccount::deserialize (&mut &**multi_sig_account.data.borrow_mut()).unwrap_or_default();
        if multi_sig_data_obj.account_state != AccountState::Initialized {
            msg!("multi sig account uninitialized");
            return Err(SigError::AccountUnInitialized.into())
        }

        let mut proposal_data_obj: ProposalAccount = ProposalAccount::deserialize (&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();
        if proposal_data_obj.account_state != AccountState::Initialized ||
            proposal_data_obj.tx_expired_time < clock::Clock::get().unwrap().unix_timestamp  {
            msg!("proposal account uninitialized or expired");
            return Err(SigError::AccountUnInitialized.into())
        }

        if !multi_sig_data_obj.is_signer(signer_account.key) {
            msg!("singer account has no authority");
            return Err(SigError::NoAuthority.into())
        }
        let (voted, suitable_index) =  proposal_data_obj.is_voted_and_get_suitable_place(&signer_account.key);
        if voted {
            msg!("singer was already voted");
            return Err(SigError::RepeatVote.into())
        }

        proposal_data_obj.tickets[suitable_index] = (*signer_account.key, true);
        proposal_data_obj.serialize(&mut *proposal_account.data.borrow_mut())?;

        msg!("vote success");
        return Ok(())
    }

    /// verify proposal state
    fn _verify(_: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, summary_plain: Option<String>) -> ProgramResult{
        let multi_sig_account = next_account_info(accounts_iter)?;
        let sender_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;

        let multi_sig_data_obj : MultiSigAccount =
            MultiSigAccount::deserialize (&mut &**multi_sig_account.data.borrow_mut()).unwrap_or_default();
        if multi_sig_data_obj.account_state != AccountState::Initialized {
            msg!("multi sig account uninitialized");
            return Err(SigError::AccountUnInitialized.into())
        }

        let mut proposal_data_obj: ProposalAccount = ProposalAccount::deserialize (&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();
        // Uninitialized state or expired state not verify
        if proposal_data_obj.account_state != AccountState::Initialized ||
            clock::Clock::get().unwrap().unix_timestamp > proposal_data_obj.tx_expired_time {
            msg!("proposal account uninitialized or expired");
            return Err(SigError::AccountUnInitialized.into())
        }
        if proposal_data_obj.initiator != *sender_account.key {
            msg!("no authority verify");
            return Err(SigError::NoAuthority.into())
        }
        if summary_plain.is_some() {
            let summary = summary_plain.unwrap().add(format!(", nonce {}", proposal_data_obj.nonce).as_str());
            if summary != proposal_data_obj.summary {
                msg!("summary unmatched with before");
                return Err(SigError::SummaryUnmatched.into())
            }
        }

        let mut agree_tickets = 0;
        proposal_data_obj
            .tickets
            .iter()
            .find(|(_, agree)| {
                if !agree {
                    return true
                }
                agree_tickets += 1;
                false
            });
        let total_valid_tickets = multi_sig_data_obj.get_valid_tickets_count();
        if agree_tickets * 100 < multi_sig_data_obj.threshold as usize * total_valid_tickets {
            msg!("proposal not pass yet");
            return Err(SigError::NotPass.into())
        }
        proposal_data_obj.account_state = AccountState::Verified;
        proposal_data_obj.serialize(&mut *proposal_account.data.borrow_mut())?;

        msg!("proposal verified");
        return Ok(())
    }

    /// Add new singer
    fn _add_signer(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, signer: Pubkey) -> ProgramResult{
        let mut accounts_iter_cl = accounts_iter.clone();

        let multi_sig_account = next_account_info(accounts_iter)?;
        let initiator_account = next_account_info(accounts_iter)?;
        let _proposal_account = next_account_info(accounts_iter)?;
        let _system_program_account = next_account_info(accounts_iter)?;
        let _rent_account = next_account_info(accounts_iter)?;


        let custom_check = move |multi_sig_account_obj :&MultiSigAccount| -> ProgramResult {
            let (existed, _) = multi_sig_account_obj.is_signer_and_get_seat(&signer);
            if existed {
                msg!("Singer already exist.");
                return Err(SigError::SingerAlreadyExist.into())
            }
            Ok(())
        };

        let build_proposal = move |_ :&MultiSigAccount| -> String {
            format!("{} initiating add a new signer {} to multi sig account {}",
                    initiator_account.key, signer, multi_sig_account.key)
        };

        let custom_logic_after_verify = move |multi_sig_account_obj :&mut MultiSigAccount| -> ProgramResult {
            // Add new signer
            let _ = multi_sig_account_obj.add_new(signer);
            multi_sig_account_obj.serialize(&mut *multi_sig_account.data.borrow_mut())?;
            Ok(())
        };

        Self::init_proposal_or_verify(
            program_id,
            &mut accounts_iter_cl,
            custom_check,
            build_proposal,
            custom_logic_after_verify
        )?;

        return Ok(())
    }

    /// Delete singer
    fn _remove_signer(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, signer: Pubkey) -> ProgramResult{
        let mut accounts_iter_cl = accounts_iter.clone();

        let multi_sig_account = next_account_info(accounts_iter)?;
        let initiator_account = next_account_info(accounts_iter)?;
        let _proposal_account = next_account_info(accounts_iter)?;
        let _system_program_account = next_account_info(accounts_iter)?;
        let _rent_account = next_account_info(accounts_iter)?;

        let custom_check = move |multi_sig_account_obj :&MultiSigAccount| -> ProgramResult {
            let (is_signer, _) = multi_sig_account_obj.is_signer_and_get_seat(&signer);
            if !is_signer {
                msg!("{} is not singer", signer);
                return Err(SigError::NotASinger.into())
            }
            if multi_sig_account_obj.get_valid_singers_count() <= 1 {
                msg!("signers too less, can not remove");
                return Err(SigError::CanNotRemove.into())
            }

            Ok(())
        };
        let build_proposal = move |_ :&MultiSigAccount| -> String {
            format!("{} initiating remove a signer {} from multi sig account {}",
                    initiator_account.key, signer, multi_sig_account.key)
        };

        let custom_logic_after_verify = move |multi_sig_account_obj: &mut MultiSigAccount| -> ProgramResult {
            // Remove a signer
            let (_, seat) = multi_sig_account_obj.is_signer_and_get_seat(&signer);
            multi_sig_account_obj.accounts[seat] = (Pubkey::default(), false);
            multi_sig_account_obj.serialize(&mut *multi_sig_account.data.borrow_mut())?;
            Ok(())
        };

        Self::init_proposal_or_verify(
            program_id,
            &mut accounts_iter_cl,
            custom_check,
            build_proposal,
            custom_logic_after_verify
        )?;

        return Ok(())
    }

    /// Close proposal
    fn _close_proposal(_: &Pubkey, accounts_iter: &mut Iter<AccountInfo>) -> ProgramResult{
        let initiator_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;
        let multi_sig_account = next_account_info(accounts_iter)?;

        let proposal_data_obj: ProposalAccount =  ProposalAccount::deserialize (&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();

        if proposal_data_obj.account_state == AccountState::Uninitialized {
            msg!("Proposal account not initialized");
            return Err(SigError::AccountUnInitialized.into())
        }

        if proposal_data_obj.account_state == AccountState::Initialized &&
            clock::Clock::get().unwrap().unix_timestamp > proposal_data_obj.tx_expired_time {
            msg!("Proposal can not be close cause unexpired");
            return Err(SigError::CanNotClose.into())
        }

        let multi_sig_data_obj : MultiSigAccount =
            MultiSigAccount::deserialize (&mut &**multi_sig_account.data.borrow_mut()).unwrap_or_default();

        if multi_sig_data_obj.account_state == AccountState::Uninitialized {
            msg!("multi sig account uninitialized");
            return Err(SigError::AccountUnInitialized.into())
        }

        if !pubkeys_equal(&initiator_account.key, &proposal_data_obj.initiator) {
            msg!("Not a initiator to close this proposal");
            return Err(SigError::NoAuthToClose.into())
        }

        if !multi_sig_data_obj.is_signer(initiator_account.key) {
            msg!("Not a signer");
            return Err(SigError::NotASinger.into())
        }


        // Clean proposal account.
        let initiator_account_lamports = initiator_account.lamports();
        **initiator_account.lamports.borrow_mut() = initiator_account_lamports
            .checked_add(proposal_account.lamports())
            .ok_or(ProgramError::InvalidArgument)?;

        **proposal_account.lamports.borrow_mut() = 0;
        let data_len = proposal_account.data_len();
        put_memset(*proposal_account.data.borrow_mut(), 0, data_len);

        return Ok(())
    }

    /// Set threshold
    fn _set_threshold(program_id: &Pubkey, accounts_iter: &mut Iter<AccountInfo>, new_threshold: u8) -> ProgramResult{
        let mut accounts_iter_cl = accounts_iter.clone();

        let multi_sig_account = next_account_info(accounts_iter)?;
        let initiator_account = next_account_info(accounts_iter)?;
        let _proposal_account = next_account_info(accounts_iter)?;
        let _system_program_account = next_account_info(accounts_iter)?;
        let _rent_account = next_account_info(accounts_iter)?;


        let custom_check = move |multi_sig_account_obj :&MultiSigAccount| -> ProgramResult {
            if multi_sig_account_obj.threshold == new_threshold {
                msg!("multi sig account uninitialized");
                return Err(SigError::AccountUnInitialized.into())
            }
            Ok(())
        };

        let build_proposal = move |multi_sig_account_obj :&MultiSigAccount| -> String {
            format!("{} initiating change threshold from {} to {}",
                    initiator_account.key, multi_sig_account_obj.threshold, new_threshold)
        };

        let custom_logic_after_verify = move |multi_sig_account_obj: &mut MultiSigAccount| -> ProgramResult {
            // Set a new threshold
            multi_sig_account_obj.threshold = new_threshold;
            multi_sig_account_obj.serialize(&mut *multi_sig_account.data.borrow_mut())?;
            Ok(())
        };

        Self::init_proposal_or_verify(
            program_id,
            &mut accounts_iter_cl,
            custom_check,
            build_proposal,
            custom_logic_after_verify
        )?;

        return Ok(())
    }

    /// Create a new account, account shall be generated by PDA.
    /// account_infos:
    ///     1、new_account;
    ///     2、payer_account;
    ///     3、rent_account;
    ///     4、system_account;
    pub fn create_pda_account_with_seeds(
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

        // Alloc account space.
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

    /// Find proposal account by proposal summary and initiator.
    fn init_proposal_or_verify<
        CC: Fn(&MultiSigAccount) -> ProgramResult,
        BP: Fn(&MultiSigAccount) -> String,
        MCS: FnMut(&mut MultiSigAccount) -> ProgramResult
    >(
        program_id: &Pubkey,
        accounts_iter: &mut Iter<AccountInfo>,
        custom_check: CC,
        build_proposal: BP,
        mut maybe_change_state: MCS
    ) -> ProgramResult {
        let mut accounts_iter_cl = accounts_iter.clone();

        let multi_sig_account = next_account_info(accounts_iter)?;
        let initiator_account = next_account_info(accounts_iter)?;
        let proposal_account = next_account_info(accounts_iter)?;

        let mut multi_sig_data_obj : MultiSigAccount =
            MultiSigAccount::deserialize (&mut &**multi_sig_account.data.borrow_mut()).unwrap_or_default();

        if multi_sig_data_obj.account_state == AccountState::Uninitialized {
            msg!("multi sig account uninitialized");
            return Err(SigError::AccountUnInitialized.into())
        }

        if !multi_sig_data_obj.is_signer(initiator_account.key) {
            msg!("no authority");
            return Err(SigError::NoAuthority.into())
        }
        custom_check(&multi_sig_data_obj)?;
        let proposal= build_proposal(&multi_sig_data_obj);

        let proposal_data_obj: ProposalAccount =  ProposalAccount::deserialize (&mut &**proposal_account.data.borrow_mut()).unwrap_or_default();
        return if proposal_data_obj.account_state == AccountState::Uninitialized {
            Self::_init_proposal_account(program_id, &mut accounts_iter_cl, multi_sig_account.key.clone(), proposal, PROPOSAL_EFFECT_PERIOD as u32)?;
            Ok(())
        } else {
            Self::_verify(program_id, &mut accounts_iter_cl, Some(proposal))?;
            maybe_change_state(&mut multi_sig_data_obj)?;
            Ok(())
        }
    }


    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        msg!("start to process");
        let instruction = SigInstruction::deserialize(input).unwrap();
        let mut accounts_iter = accounts.iter();
        match instruction {
            SigInstruction::CreateMultiSigAccount { accounts, threshold} => {
                msg!("start to process CreateMultiSigAccount instruction");
                Self::_create_multi_sig_account(program_id, &mut accounts_iter, accounts, threshold)
            }

            SigInstruction::CreateInitProposalAccount { parent_key, summary, continuous_validity_period} => {
                msg!("start to process CreateInitProposalAccount instruction");
                Self::_init_proposal_account(program_id, &mut accounts_iter, parent_key, summary, continuous_validity_period)
            }

            SigInstruction::Vote => {
                msg!("start to process Vote instruction");
                Self::_vote(program_id, &mut accounts_iter)
            }

            SigInstruction::Verify { summary } => {
                msg!("start to process Verify instruction");
                Self::_verify(program_id, &mut accounts_iter, summary)
            }

            SigInstruction::AddSigner{ signer} => {
                msg!("start to process AddSigner instruction");
                Self::_add_signer(program_id, &mut accounts_iter, signer)
            }

            SigInstruction::RemoveSigner { signer } => {
                msg!("start to process RemoveSigner instruction");
                Self::_remove_signer(program_id, &mut accounts_iter, signer)
            }
            SigInstruction::CloseProposalAccount => {
                msg!("start to process CloseProposalAccount instruction");
                Self::_close_proposal(program_id, &mut accounts_iter)
            }
            SigInstruction::SetMultiSigThreshold { new_threshold } => {
                msg!("start to process SetMultiSigThreshold instruction");
                Self::_set_threshold(program_id, &mut accounts_iter, new_threshold)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use put_program::entrypoint::ProgramResult;
    use put_program::instruction::Instruction;
    use put_program::pubkey::Pubkey;
    use put_sdk::account::{Account as PUTAccount, create_is_signer_account_infos};
    use crate::error::SigError;
    use crate::instruction::create_vote_ins;
    use crate::processor::Processor;
    use crate::state::{AccountState, MultiSigAccount, ProposalAccount};

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
    fn test_vote() {
        let program_id = crate::id();
        let multi_sig_account_puk = Pubkey::new_unique();
        let proposal_account_puk = Pubkey::new_unique();
        let singer_account_puk1 = Pubkey::new_unique();
        let singer_account_puk2 = Pubkey::new_unique();

        let mut multi_sig_account_obj = MultiSigAccount{
            account_state: Default::default(),
            accounts: [(Pubkey::default(), false); 10],
            threshold: 51,
            nonce: 0
        };

        multi_sig_account_obj.add_new(singer_account_puk1.clone());
        multi_sig_account_obj.add_new(singer_account_puk2.clone());

        let multi_sig_account_data = borsh::to_vec(&multi_sig_account_obj).unwrap();
        let mut multi_sig_account = PUTAccount::new(10, multi_sig_account_data.len(), &program_id);
        multi_sig_account.data = multi_sig_account_data.clone();

        let mut proposal_account_obj = ProposalAccount{
           account_state: Default::default(),
           tickets: [(Pubkey::default(), false); 10],
           parent: Default::default(),
           tx_expired_time: 0,
           initiator: Default::default(),
           summary: "".to_string(),
           nonce: 0
        };


        let proposal_account_data = borsh::to_vec(&proposal_account_obj).unwrap();
        let mut proposal_account = PUTAccount::new(10, proposal_account_data.len(), &program_id);
        proposal_account.data = proposal_account_data.clone();

        let mut signer1_account = PUTAccount::default();

        // AccountNotExist
        assert_eq!(
            Err(SigError::AccountUnInitialized.into()),
            do_process_instruction(
                create_vote_ins(&program_id, &multi_sig_account_puk, &proposal_account_puk, &singer_account_puk1).unwrap(),
                vec![&mut multi_sig_account, &mut proposal_account, &mut signer1_account]
            )
        );

        multi_sig_account_obj.account_state = AccountState::Initialized;
        let multi_sig_account_data = borsh::to_vec(&multi_sig_account_obj).unwrap();
        multi_sig_account.data = multi_sig_account_data.clone();

        // AccountNotExist
        assert_eq!(
            Err(SigError::AccountUnInitialized.into()),
            do_process_instruction(
                create_vote_ins(&rogram_id, &multi_sig_account_puk, &proposal_account_puk, &singer_account_puk1).unwrap(),
                vec![&mut multi_sig_account, &mut proposal_account, &mut signer1_account]
            )
        );
    }
}