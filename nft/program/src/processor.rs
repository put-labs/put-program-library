//! Program state processor

use crate::{
    error::TokenError,
    instruction::{TokenInstruction},
    state::{MetaAccount, AccountState, NftMint},
};
use num_traits::FromPrimitive;
use put_program::{account_info::{next_account_info, AccountInfo}, decode_error::DecodeError, entrypoint::ProgramResult, msg, program_error::{PrintProgramError, ProgramError}, program_memory::{put_memcmp}, pubkey::{Pubkey, PUBKEY_BYTES}, system_instruction, sysvar::{rent::Rent, Sysvar}};
use put_program::program::{invoke, invoke_signed};
use put_program::program_memory::put_memset;
use put_program::program_pack::Pack;
use crate::instruction::{AuthorityType, SetAuthorityArgs, UpdateType};
use crate::state::{MAX_META_DATA_SIZE, MINT_SIZE};


/// Program state handler.
pub struct Processor {}
impl Processor {
    fn _process_initialize_mint(
        program_id: Pubkey,
        accounts: &[AccountInfo],
        total_supply: u64,
        mint_authority: Pubkey,
        freeze_authority: Option<Pubkey>,
        name: String,
        symbol: String,
        icon_uri: String
    ) -> ProgramResult {

        // 1、Load accounts
        let account_info_iter = &mut accounts.iter();
        let mint_account_info = next_account_info(account_info_iter)?;
        let payer_account_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let rent_info = next_account_info(account_info_iter)?;


        // 2、Check  whether account initialized.
        if !mint_account_info.data_is_empty() {
            return Err(TokenError::AlreadyInUse.into());
        }

        // 3、init mint data
        let mut mint = NftMint::default();
        mint.mint_authority = mint_authority;
        mint.supply = 0;
        mint.total_supply = total_supply;
        mint.is_initialized = true;
        mint.freeze_authority = freeze_authority;
        mint.name = name.clone();
        mint.symbol = symbol.clone();
        mint.icon_uri = icon_uri;

        // 4、pay for rent
        msg!("staring minus the rent for mint account");
        let rent = &Rent::from_account_info(rent_info)?;
        let required_lamports = rent
            .minimum_balance(MINT_SIZE)
            .max(1)
            .saturating_sub(mint_account_info.lamports());

        if required_lamports > 0 {
            msg!("Transfer {} lamports to the new account", required_lamports);
            invoke(
                &system_instruction::transfer(payer_account_info.key, mint_account_info.key, required_lamports),
                &[
                    payer_account_info.clone(),
                    mint_account_info.clone(),
                    system_program_info.clone(),
                ],
            )?;
        }

        // 5、alloc space for new account
        let allocate_accounts = &[mint_account_info.clone(), system_program_info.clone()];
        msg!("Allocate space for the account");
        let signer_seeds = &[
            name.as_bytes(),
            symbol.as_bytes(),
            program_id.as_ref(),
            mint_account_info.key.as_ref(),
        ];
        let (_, metadata_bump_seed) =
            Pubkey::find_program_address(signer_seeds, &program_id);

        let metadata_signer_seeds = &[
            name.as_bytes(),
            symbol.as_bytes(),
            program_id.as_ref(),
            mint_account_info.key.as_ref(),
            &[metadata_bump_seed],
        ];
        invoke_signed(
            &system_instruction::allocate(mint_account_info.key, MINT_SIZE as u64),
            allocate_accounts,
            &[metadata_signer_seeds],
        )?;


        msg!("Assign the account to the owning program");
        invoke_signed(
            &system_instruction::assign(mint_account_info.key, &program_id),
            allocate_accounts,
            &[metadata_signer_seeds],
        )?;

        // 6、save data
        let mut mint_account_info_data = mint_account_info.try_borrow_mut_data().unwrap();
        NftMint::pack(mint, &mut mint_account_info_data)?;

        Ok(())
    }

    /// Processes an [InitializeMint](enum.TokenInstruction.html) instruction.
    pub fn process_initialize_mint(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        total_supply: u64,
        mint_authority: Pubkey,
        freeze_authority: Option<Pubkey>,
        name: String,
        symbol: String,
        icon_uri: String
    ) -> ProgramResult {
        Self::_process_initialize_mint(*program_id, accounts, total_supply, mint_authority, freeze_authority, name, symbol, icon_uri)
    }

    // mint to
    fn _process_mint_to(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        owner: Option<&Pubkey>,
        token_uri: String,
        _rent_sysvar_account: bool,
    ) -> ProgramResult {

        // 1、load accounts
        let account_info_iter = &mut accounts.iter();
        let nft_account_info = next_account_info(account_info_iter)?;
        let mint_account_info = next_account_info(account_info_iter)?;
        let payer_account_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;
        let rent_account_info = next_account_info(account_info_iter)?;
        let owner = if let Some(owner) = owner {
            owner
        } else {
            payer_account_info.key
        };


        // 2、check authority
        if !Self::cmp_pubkeys(program_id, &mint_account_info.owner) {
            return Err(TokenError::OwnerMismatch.into());
        }
        // will check init.
        let mut mint = NftMint::unpack(&mint_account_info.data.borrow())?;
        if mint.supply == mint.total_supply {
            return Err(TokenError::AlreadyReachMaxMintNum.into());
        }


        // 3、init nft meta
        let mut meta = MetaAccount::default();

        meta.mint = *mint_account_info.key;
        meta.owner = *owner;
        meta.close_authority = None;
        meta.state = AccountState::Initialized;
        meta.token_id = mint.supply + 1;
        meta.token_uri = token_uri.clone();


        // 4、pay for rent
        msg!("staring minus the rent for the nft account");
        let rent = &Rent::from_account_info(rent_account_info)?;
        let required_lamports = rent
            .minimum_balance(MAX_META_DATA_SIZE)
            .max(1)
            .saturating_sub(nft_account_info.lamports());

        if required_lamports > 0 {
            msg!("Transfer {} lamports to the new account", required_lamports);
            invoke(
                &system_instruction::transfer(payer_account_info.key, nft_account_info.key, required_lamports),
                &[
                    payer_account_info.clone(),
                    nft_account_info.clone(),
                    system_program_info.clone(),
                ],
            )?;
        }

        // 5、 alloc account space.
        let index = (meta.token_id as u64).to_le_bytes();
        let signer_seeds = &[
            index.as_ref(),
            program_id.as_ref(),
            mint_account_info.key.as_ref()
        ];
        let (_, metadata_bump_seed) =
            Pubkey::find_program_address(signer_seeds, &program_id);
        let metadata_signer_seeds = &[
            index.as_ref(),
            program_id.as_ref(),
            mint_account_info.key.as_ref(),
            &[metadata_bump_seed],
        ];
        let allocate_accounts = &[nft_account_info.clone(), system_program_info.clone()];

        msg!("Allocate space for the account");
        invoke_signed(
            &system_instruction::allocate(nft_account_info.key, MAX_META_DATA_SIZE as u64),
            allocate_accounts,
            &[metadata_signer_seeds]
        )?;

        msg!("Assign the account to the owning program");
        invoke_signed(
            &system_instruction::assign(nft_account_info.key, &program_id),
            allocate_accounts,
            &[metadata_signer_seeds],
        )?;

        // 6、save meta data, will fill 0 if data not enough big.
        msg!("saving nft meta data");
        MetaAccount::pack(meta, &mut nft_account_info.data.borrow_mut())?;
        // meta.serialize(&mut meta_data_in_account.as_ref())?;


        // 7、update mint data and save
        msg!("updating nft mint data");
        mint.supply += 1;
        NftMint::pack(mint, &mut mint_account_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes an [InitializeAccount](enum.TokenInstruction.html) instruction.
    pub fn process_mint_to(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        token_uri: String,
    ) -> ProgramResult {
        Self::_process_mint_to(program_id, accounts, None, token_uri, true)
    }


    /// Processes a [Transfer](enum.TokenInstruction.html) instruction.
    pub fn process_transfer(
        _: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        // 1, get accounts
        let account_info_iter = &mut accounts.iter();

        let source_account_info = next_account_info(account_info_iter)?;
        let destination_account_info = next_account_info(account_info_iter)?;
        let nft_account_info = next_account_info(account_info_iter)?;

        // 2、get NFT meta data, and checking account information is correct
        let mut meta_data = nft_account_info.data.try_borrow_mut().unwrap();
        let mut nft_meta = MetaAccount::unpack( meta_data.as_ref())?;

        if nft_meta.is_frozen() {
            msg!("Account Frozen.");
            return Err(TokenError::AccountFrozen.into());
        }
        // Check NFT owner is correct.
        if !Self::cmp_pubkeys(&nft_meta.owner, &source_account_info.key) {
            msg!("Owner mismatch.");
            return Err(TokenError::OwnerMismatch.into());
        }

        // 3、Inspection of NFT rotation
        let self_transfer =
            Self::cmp_pubkeys(source_account_info.key, destination_account_info.key);

        // This check MUST occur just before the amounts are manipulated
        // to ensure self-transfers are fully validated
        if self_transfer {
            msg!("self transfer.");
            return Ok(());
        }

        // 4、To modify nft ownership
        msg!("changing the nft[{}] owner from[{}] to[{}]", nft_account_info.key, source_account_info.key, destination_account_info.key);
        nft_meta.owner = *destination_account_info.key;
        // let mut nft_meta_data = nft_meta.serialize_to().unwrap();
        // let mut empty_tail = vec![0 as u8; MAX_META_DATA_SIZE - nft_meta_data.len()];
        // nft_meta_data.append(&mut empty_tail);
        // meta_data.copy_from_slice(nft_meta_data.as_slice());
        MetaAccount::pack(nft_meta,&mut meta_data)
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = TokenInstruction::deserialize(input).unwrap();

        match instruction {
            TokenInstruction::InitializeMint(args) => {
                msg!("Instruction: InitializeMint");
                Self::process_initialize_mint(program_id, accounts, args.total_supply, args.mint_authority, args.freeze_authority, args.name, args.symbol, args.icon_uri)
            }

            TokenInstruction::MintTo {uri} => {
                msg!("Instruction: InitializeAccount");
                Self::process_mint_to(program_id, accounts, uri)
            }

            TokenInstruction::Update (update_type) => {
                msg!("Instruction: UpdateNftUri");
                //1、load accounts.
                let account_info_iter = &mut accounts.iter();
                let address_account_info = next_account_info(account_info_iter)?;
                let owner_account = next_account_info(account_info_iter)?;

                match update_type {
                    UpdateType::Icon {icon_uri} => {
                        // 2、get nft meta_data, and check whether the meta account init.
                        let mut mint = NftMint::unpack( &address_account_info.data.borrow_mut())?;

                        // check authority.
                        if !Self::cmp_pubkeys(&mint.mint_authority, &owner_account.key) {
                            return Err(TokenError::OwnerMismatch.into());
                        }

                        mint.icon_uri = icon_uri;
                        NftMint::pack( mint,&mut address_account_info.data.borrow_mut())
                    }
                    UpdateType::NftAsset {token_uri} => {
                        // 2、get nft meta_data, and check whether the meta account init.
                        let mut nft_meta = MetaAccount::unpack( &address_account_info.data.borrow_mut())?;

                        // check authority.
                        if !Self::cmp_pubkeys(&nft_meta.owner, &owner_account.key) {
                            return Err(TokenError::OwnerMismatch.into());
                        }

                        nft_meta.token_uri = token_uri;
                        MetaAccount::pack( nft_meta,&mut address_account_info.data.borrow_mut())
                    }
                }
            }
            TokenInstruction::Transfer => {
                msg!("Instruction: Transfer");
                Self::process_transfer(program_id, accounts)
            }

            TokenInstruction::Freeze => {
                msg!("Instruction: Freeze");
                //1、Load accounts
                let account_info_iter = &mut accounts.iter();
                let nft_account_info = next_account_info(account_info_iter)?;
                let authority_account = next_account_info(account_info_iter)?;
                let mint_account = next_account_info(account_info_iter)?;

                //2、Check the account validity.
                let mint_data = mint_account.data.try_borrow().unwrap();
                let mint = NftMint::unpack(&mint_data)?;
                let mut nft_meta_data = nft_account_info.data.try_borrow_mut().unwrap();
                let mut meta = MetaAccount::unpack(&nft_meta_data)?;

                let mut authority_pubkey = mint.mint_authority;
                if let Some(freeze_authority) = mint.freeze_authority {
                    authority_pubkey = freeze_authority
                }
                if !Self::cmp_pubkeys(&authority_pubkey, &authority_account.key) {
                    return Err(TokenError::AuthorityMismatched.into())
                }
                if !Self::cmp_pubkeys(&meta.mint, &mint_account.key) {
                    return Err(TokenError::MintMismatch.into())
                }

                if meta.state == AccountState::Frozen {
                    return Err(TokenError::AlreadyFrozen.into())
                }

                // Change the account status.
                meta.state = AccountState::Frozen;
                MetaAccount::pack(meta, &mut nft_meta_data)
            }

            TokenInstruction::Thaw => {
                msg!("Instruction: Thaw");
                // 1、load accounts
                let account_info_iter = &mut accounts.iter();
                let nft_account_info = next_account_info(account_info_iter)?;
                let authority_account = next_account_info(account_info_iter)?;
                let mint_account = next_account_info(account_info_iter)?;

                // 2、Check whether account valid.
                let mint_data = mint_account.data.try_borrow().unwrap();
                let mint = NftMint::unpack(&mint_data)?;
                let mut nft_meta_data = nft_account_info.data.try_borrow_mut().unwrap();
                let mut meta = MetaAccount::unpack(&nft_meta_data)?;

                let mut authority_pubkey = mint.mint_authority;
                if let Some(freeze_authority) = mint.freeze_authority {
                    authority_pubkey = freeze_authority
                }
                if !Self::cmp_pubkeys(&authority_pubkey, &authority_account.key) {
                    return Err(TokenError::AuthorityMismatched.into())
                }
                if !Self::cmp_pubkeys(&meta.mint, &mint_account.key) {
                    return Err(TokenError::MintMismatch.into())
                }

                if meta.state != AccountState::Frozen {
                    return Err(TokenError::ThawUnfrozen.into())
                }

                // Modify account state.
                meta.state = AccountState::Initialized;
                MetaAccount::pack(meta, &mut nft_meta_data)
            }

            TokenInstruction::Burn => {
                msg!("Instruction: Burn");
                // 1、load accounts
                let account_info_iter = &mut accounts.iter();
                let nft_account_info = next_account_info(account_info_iter)?;
                let close_auth_account = next_account_info(account_info_iter)?;

                // 2、Check whether account valid.
                let meta = MetaAccount::unpack(&nft_account_info.data.borrow())?;

                let mut authority_pubkey = meta.owner;
                if let Some(close_authority) = meta.close_authority {
                    authority_pubkey = close_authority
                }
                if !Self::cmp_pubkeys(&authority_pubkey, &close_auth_account.key) {
                    return Err(TokenError::AuthorityMismatched.into())
                }
                if meta.state == AccountState::Frozen {
                    return Err(TokenError::AccountFrozen.into())
                }

                let close_auth_account_balance_lamports = close_auth_account.lamports();
                **close_auth_account.lamports.borrow_mut() = close_auth_account_balance_lamports
                    .checked_add(nft_account_info.lamports())
                    .ok_or(TokenError::Overflow)?;

                // nft_account_info set lamports to 0，it will be clean
                **nft_account_info.lamports.borrow_mut() = 0;
                put_memset(*nft_account_info.data.borrow_mut(), 0, MetaAccount::LEN);
                Ok(())
            }

            TokenInstruction::SetAuthority (sea)  => {
                let SetAuthorityArgs{authority_type, new_authority}  = sea;
                msg!("Instruction: SetAuthority");
                // 1、load accounts
                let account_info_iter = &mut accounts.iter();
                let target_account = next_account_info(account_info_iter)?;
                let old_owner_account = next_account_info(account_info_iter)?;

                match authority_type {
                    AuthorityType::MintTokens => {
                        // 2、check authority
                        let mut mint = NftMint::unpack( &target_account.data.borrow())?;
                        if !Self::cmp_pubkeys(&mint.mint_authority, &old_owner_account.key) {
                            msg!("--------- mint_authority {}", mint.mint_authority.to_string());
                            msg!("--------- old_owner_account {}", old_owner_account.key.to_string());
                            return Err(TokenError::AuthorityMismatched.into())
                        }

                        if Self::cmp_pubkeys(&mint.mint_authority, &new_authority.unwrap()) {
                            return Err(TokenError::SameAuthority.into())
                        }

                        // 3、change mint authority and save
                        mint.mint_authority = new_authority.unwrap();
                        NftMint::pack(mint, &mut target_account.data.borrow_mut())
                    }
                    AuthorityType::FreezeAccount => {
                        // 2、check authority
                        let mut mint = NftMint::unpack( &target_account.data.borrow())?;
                        if !Self::cmp_pubkeys(&mint.mint_authority, &old_owner_account.key) {
                            return Err(TokenError::AuthorityMismatched.into())
                        }

                        // 3、Compare freeze_authority with new_authority
                        // both are None
                        if mint.freeze_authority.is_none() && new_authority.is_none()  {
                            return Err(TokenError::SameAuthority.into())
                        }
                        // both exist and same
                        if mint.freeze_authority.is_some() && new_authority.is_some() &&
                                Self::cmp_pubkeys(&mint.freeze_authority.unwrap(), &new_authority.unwrap()) {
                            return Err(TokenError::SameAuthority.into())
                        }
                        // 4、Change freeze authority and save data
                        mint.freeze_authority = new_authority;
                        NftMint::pack(mint, &mut target_account.data.borrow_mut())
                    }
                    AuthorityType::CloseAccount => {
                        // 2、check authority
                        let mut meta = MetaAccount::unpack( &target_account.data.borrow())?;
                        if !Self::cmp_pubkeys(&meta.owner, &old_owner_account.key) {
                            return Err(TokenError::AuthorityMismatched.into())
                        }

                        // 3、Compare close_authority with new_authority
                        // both are None
                        if meta.close_authority.is_none() && new_authority.is_none() {
                            return Err(TokenError::SameAuthority.into())
                        }
                        if (meta.close_authority.is_some() && new_authority.is_some()) &&
                                Self::cmp_pubkeys(&meta.close_authority.unwrap(),&new_authority.unwrap()) {
                            return Err(TokenError::SameAuthority.into())
                        }
                        // 4、Change close authority and save data
                        meta.close_authority = new_authority;

                        MetaAccount::pack(meta, &mut target_account.data.borrow_mut())
                    }
                }
            }
        }
    }

    /// Checks that the account is owned by the expected program
    pub fn check_account_owner(program_id: &Pubkey, account_info: &AccountInfo) -> ProgramResult {
        if !Self::cmp_pubkeys(program_id, account_info.owner) {
            Err(ProgramError::IncorrectProgramId)
        } else {
            Ok(())
        }
    }

    /// Checks two pubkeys for equality in a computationally cheap way using
    /// `sol_memcmp`
    pub fn cmp_pubkeys(a: &Pubkey, b: &Pubkey) -> bool {
        put_memcmp(a.as_ref(), b.as_ref(), PUBKEY_BYTES) == 0
    }


}

impl PrintProgramError for TokenError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            TokenError::MintMismatch => msg!("Error: Account not associated with this Mint"),
            TokenError::OwnerMismatch => msg!("Error: Owner not matched"),
            TokenError::AlreadyInUse => msg!("Error: account or token already in use"),
            TokenError::UninitializedState => msg!("Error: State is uninitialized"),
            TokenError::Overflow => msg!("Error: Operation overflowed"),

            TokenError::AccountFrozen => msg!("Error: Account is frozen"),

            TokenError::AlreadyReachMaxMintNum => {
                msg!("Error: token of mint already reach max_supply count")
            }
            TokenError::NameIsTooLen => {
                msg!("Error: len of name is gt max_len")
            }
            TokenError::SymbolIsTooLen => {
                msg!("Error: len of symbol is gt max_len")
            }
            TokenError::UriIsTooLen => {
                msg!("Error: len of uri is gt max_len")
            }
            TokenError::AuthorityMismatched => {
                msg!("Error: authority mismatched")
            }
            TokenError::SameAuthority => {
                msg!("Error: set authority with same account")
            }
            TokenError::ThawUnfrozen => {
                msg!("Error: thaw a unfrozen nft")
            }
            #[warn(unreachable_patterns)]
            _ => {unreachable!()}
        }
    }
}


#[cfg(test)]
mod tests {
    use put_program::entrypoint::ProgramResult;
    use put_program::instruction::Instruction;
    use put_program::program_error::ProgramError;
    use put_program::program_pack::Pack;
    use put_program::pubkey::Pubkey;
    use put_sdk::account::{create_is_signer_account_infos};
    use crate::instruction::{AuthorityType, create_authorize_instruction, create_burn_instruction, create_freeze_instruction, create_thaw_instruction, create_transfer_inst};
    use crate::processor::Processor;
    use crate::state::{AccountState, MAX_META_DATA_SIZE, MetaAccount, MINT_SIZE, NftMint};
    use put_sdk::account::Account as PUTAccount;
    use crate::error::TokenError;

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
    fn test_transfer() {
        let program_id = crate::id();
        let from_account_puk = Pubkey::new_unique();
        let to_account_puk = Pubkey::new_unique();
        let nft_account_puk = Pubkey::new_unique();

        let mut nft_account_data_obj = MetaAccount{
            mint: Default::default(),
            owner: Default::default(),
            state: Default::default(),
            close_authority: None,
            token_id: 0,
            token_uri: "".to_string()
        };

        let mut nft_account_data = [0u8; MAX_META_DATA_SIZE];
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);


        let mut nft_account = PUTAccount::new(1000, MAX_META_DATA_SIZE, &program_id);
        nft_account.data = nft_account_data.to_vec();

        let mut from_account = PUTAccount::default();
        let mut to_account = PUTAccount::default();

        // AccountNotExist
        assert_eq!(
            Err(ProgramError::UninitializedAccount),
            do_process_instruction(
                create_transfer_inst(from_account_puk, to_account_puk, nft_account_puk, program_id).unwrap(),
                vec![&mut from_account, &mut to_account, &mut nft_account]
            )
        );

        nft_account_data_obj.state = AccountState::Initialized;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        assert_eq!(
            Err(TokenError::OwnerMismatch.into()),
            do_process_instruction(
                create_transfer_inst(from_account_puk, to_account_puk, nft_account_puk, program_id).unwrap(),
                vec![&mut from_account, &mut to_account, &mut nft_account]
            )
        );
        nft_account_data_obj.owner = from_account_puk;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        do_process_instruction(
            create_transfer_inst(from_account_puk, to_account_puk, nft_account_puk, program_id).unwrap(),
            vec![&mut from_account, &mut to_account, &mut nft_account]
        );

        let transfer_after_data_obj = MetaAccount::unpack(&nft_account.data).unwrap();
        assert_eq!(transfer_after_data_obj.owner, to_account_puk);
    }

    #[test]
    fn test_freeze() {
        let program_id = crate::id();
        let freeze_authority_puk = Pubkey::new_unique();
        let mint_account_puk = Pubkey::new_unique();
        let nft_account_puk = Pubkey::new_unique();

        let mut mint_account_data_obj = NftMint {
            mint_authority: Default::default(),
            supply: 0,
            total_supply: 0,
            is_initialized: false,
            name: "".to_string(),
            symbol: "".to_string(),
            freeze_authority: None,
            icon_uri: "".to_string()
        };

        let mut mint_account_data = [0u8; MINT_SIZE];
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        let mut mint_account = PUTAccount::new(10, MINT_SIZE, &program_id);
        mint_account.data = mint_account_data.to_vec();


        let mut nft_account_data_obj = MetaAccount{
            mint: Default::default(),
            owner: Default::default(),
            state: Default::default(),
            close_authority: None,
            token_id: 0,
            token_uri: "".to_string()
        };

        let mut nft_account_data = [0u8; MAX_META_DATA_SIZE];
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);


        let mut nft_account = PUTAccount::new(1000, MAX_META_DATA_SIZE, &program_id);
        nft_account.data = nft_account_data.to_vec();

        let mut authority_account = PUTAccount::default();

        // mint account NotExist
        assert_eq!(
            Err(ProgramError::UninitializedAccount),
            do_process_instruction(
                create_freeze_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        mint_account_data_obj.is_initialized = true;
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        mint_account.data = mint_account_data.to_vec();

        // nft Account Not Exist
        assert_eq!(
            Err(ProgramError::UninitializedAccount),
            do_process_instruction(
                create_freeze_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        nft_account_data_obj.state = AccountState::Initialized;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        assert_eq!(
            Err(TokenError::AuthorityMismatched.into()),
            do_process_instruction(
                create_freeze_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        mint_account_data_obj.freeze_authority = Some(freeze_authority_puk);
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        mint_account.data = mint_account_data.to_vec();

        assert_eq!(
            Err(TokenError::MintMismatch.into()),
            do_process_instruction(
                create_freeze_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        nft_account_data_obj.mint = mint_account_puk;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();


        do_process_instruction(
            create_freeze_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
            vec![&mut nft_account, &mut authority_account, &mut mint_account]
        );

        let nft_account_data_obj_after_freeze= MetaAccount::unpack(nft_account.data.as_slice()).unwrap();
        assert_eq!(nft_account_data_obj_after_freeze.state, AccountState::Frozen)
    }

    #[test]
    fn test_thaw() {
        let program_id = crate::id();
        let freeze_authority_puk = Pubkey::new_unique();
        let mint_account_puk = Pubkey::new_unique();
        let nft_account_puk = Pubkey::new_unique();

        let mut mint_account_data_obj = NftMint {
            mint_authority: Default::default(),
            supply: 0,
            total_supply: 0,
            is_initialized: false,
            name: "".to_string(),
            symbol: "".to_string(),
            freeze_authority: None,
            icon_uri: "".to_string()
        };

        let mut mint_account_data = [0u8; MINT_SIZE];
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        let mut mint_account = PUTAccount::new(10, MINT_SIZE, &program_id);
        mint_account.data = mint_account_data.to_vec();


        let mut nft_account_data_obj = MetaAccount{
            mint: Default::default(),
            owner: Default::default(),
            state: Default::default(),
            close_authority: None,
            token_id: 0,
            token_uri: "".to_string()
        };

        let mut nft_account_data = [0u8; MAX_META_DATA_SIZE];
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);


        let mut nft_account = PUTAccount::new(1000, MAX_META_DATA_SIZE, &program_id);
        nft_account.data = nft_account_data.to_vec();

        let mut authority_account = PUTAccount::default();

        // mint account NotExist
        assert_eq!(
            Err(ProgramError::UninitializedAccount),
            do_process_instruction(
                create_thaw_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        mint_account_data_obj.is_initialized = true;
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        mint_account.data = mint_account_data.to_vec();

        // nft Account Not Exist
        assert_eq!(
            Err(ProgramError::UninitializedAccount),
            do_process_instruction(
                create_thaw_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        nft_account_data_obj.state = AccountState::Initialized;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        // invalid freeze authority
        assert_eq!(
            Err(TokenError::AuthorityMismatched.into()),
            do_process_instruction(
                create_thaw_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        mint_account_data_obj.freeze_authority = Some(freeze_authority_puk);
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        mint_account.data = mint_account_data.to_vec();

        // nft mint unmatched
        assert_eq!(
            Err(TokenError::MintMismatch.into()),
            do_process_instruction(
                create_thaw_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        nft_account_data_obj.mint = mint_account_puk;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        // nft is unfrozen
        assert_eq!(
            Err(TokenError::ThawUnfrozen.into()),
            do_process_instruction(
                create_thaw_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account, &mut mint_account]
            )
        );

        nft_account_data_obj.state = AccountState::Frozen;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        do_process_instruction(
            create_thaw_instruction(nft_account_puk, freeze_authority_puk, mint_account_puk, program_id).unwrap(),
            vec![&mut nft_account, &mut authority_account, &mut mint_account]
        );

        let nft_account_data_obj_after_freeze= MetaAccount::unpack(nft_account.data.as_slice()).unwrap();
        assert_eq!(nft_account_data_obj_after_freeze.state, AccountState::Initialized)
    }

    #[test]
    fn test_burn() {
        let program_id = crate::id();
        let burn_authority_puk = Pubkey::new_unique();
        let nft_account_puk = Pubkey::new_unique();

        let mut nft_account_data_obj = MetaAccount{
            mint: Default::default(),
            owner: Default::default(),
            state: Default::default(),
            close_authority: None,
            token_id: 0,
            token_uri: "".to_string()
        };

        let mut nft_account_data = [0u8; MAX_META_DATA_SIZE];
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);


        let mut nft_account = PUTAccount::new(1000, MAX_META_DATA_SIZE, &program_id);
        nft_account.data = nft_account_data.to_vec();

        let mut authority_account = PUTAccount::default();

        // nft account NotExist
        assert_eq!(
            Err(ProgramError::UninitializedAccount),
            do_process_instruction(
                create_burn_instruction(nft_account_puk, burn_authority_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account]
            )
        );

        nft_account_data_obj.state = AccountState::Initialized;
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();

        // No authority
        assert_eq!(
            Err(TokenError::AuthorityMismatched.into()),
            do_process_instruction(
                create_burn_instruction(nft_account_puk, burn_authority_puk, program_id).unwrap(),
                vec![&mut nft_account, &mut authority_account]
            )
        );

        nft_account_data_obj.close_authority = Some(burn_authority_puk);
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);
        nft_account.data = nft_account_data.to_vec();


        do_process_instruction(
            create_burn_instruction(nft_account_puk, burn_authority_puk, program_id).unwrap(),
            vec![&mut nft_account, &mut authority_account]
        );

        assert_eq!(nft_account.lamports, 0);
    }

    #[test]
    fn test_set_authority() {
        let program_id = crate::id();
        let new_authority_puk = Pubkey::new_unique();
        let mint_account_puk = Pubkey::new_unique();
        let nft_account_puk = Pubkey::new_unique();
        let onwer_account_puk = Pubkey::new_unique();

        let mint_account_data_obj = NftMint {
            mint_authority: onwer_account_puk,
            supply: 0,
            total_supply: 0,
            is_initialized: true,
            name: "".to_string(),
            symbol: "".to_string(),
            freeze_authority: None,
            icon_uri: "".to_string()
        };

        let mut mint_account_data = [0u8; MINT_SIZE];
        NftMint::pack(mint_account_data_obj.clone(), &mut mint_account_data);
        let mut mint_account = PUTAccount::new(10, MINT_SIZE, &program_id);
        mint_account.data = mint_account_data.to_vec();


        let nft_account_data_obj = MetaAccount{
            mint: Default::default(),
            owner: onwer_account_puk,
            state: AccountState::Initialized,
            close_authority: None,
            token_id: 0,
            token_uri: "".to_string()
        };

        let mut nft_account_data = [0u8; MAX_META_DATA_SIZE];
        MetaAccount::pack(nft_account_data_obj.clone(), &mut nft_account_data);


        let mut nft_account = PUTAccount::new(1000, MAX_META_DATA_SIZE, &program_id);
        nft_account.data = nft_account_data.to_vec();

        let mut owner_account = PUTAccount::default();


        // mint account NotExist
        assert_eq!(
            Ok(()),
            do_process_instruction(
                create_authorize_instruction(
                    nft_account_puk,
                    Some(new_authority_puk),
                    AuthorityType::CloseAccount,
                    onwer_account_puk,
                    program_id
                ).unwrap(),
                vec![&mut nft_account, &mut owner_account]
            )
        );

        let nft_account_data_obj_after_authorize= MetaAccount::unpack(nft_account.data.as_slice()).unwrap();
        assert_eq!(nft_account_data_obj_after_authorize.close_authority, Some(new_authority_puk));

        assert_eq!(
            Ok(()),
            do_process_instruction(
                create_authorize_instruction(
                    mint_account_puk,
                    Some(new_authority_puk),
                    AuthorityType::MintTokens,
                    onwer_account_puk,
                    program_id
                ).unwrap(),
                vec![&mut mint_account, &mut owner_account]
            )
        );

        let mint_account_data_obj_after_authorize= NftMint::unpack(mint_account.data.as_slice()).unwrap();
        assert_eq!(mint_account_data_obj_after_authorize.mint_authority, new_authority_puk);


        assert_eq!(
            Ok(()),
            do_process_instruction(
                create_authorize_instruction(
                    mint_account_puk,
                    Some(new_authority_puk),
                    AuthorityType::FreezeAccount,
                    new_authority_puk,
                    program_id
                ).unwrap(),
                vec![&mut mint_account, &mut owner_account]
            )
        );

        let mint_account_data_obj_after_authorize= NftMint::unpack(mint_account.data.as_slice()).unwrap();
        assert_eq!(mint_account_data_obj_after_authorize.freeze_authority, Some(new_authority_puk));

    }
}