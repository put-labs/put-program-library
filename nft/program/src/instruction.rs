//! Instruction types

use put_program::{pubkey::Pubkey, sysvar};
use borsh::{ BorshSerialize, BorshDeserialize };
use put_program::instruction::{AccountMeta, Instruction};
use put_program::program_error::ProgramError;
use crate::check_program_account;
use shank::ShankInstruction;

/// Minimum number of multisignature signers (min N)
pub const MIN_SIGNERS: usize = 1;
/// Maximum number of multisignature signers (max N)
pub const MAX_SIGNERS: usize = 11;

/// Instructions supported by the token program.
// #[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, ShankInstruction)]
pub enum TokenInstruction {
    /// Initializes a new mint and optionally deposits all the newly minted
    /// tokens in an account.
    ///
    /// The `InitializeMint` instruction requires no signers and MUST be
    /// included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` The mint to initialize.
    ///   1. `[signer]` the authority of this mint
    ///   2. `[]` the system_program
    ///   3. `[]` Rent sysvar
    ///
    #[account(0, writable, signer, name="mint", desc="mint key")]
    #[account(1, signer, name="mint_authority", desc="Mint authority")]
    #[account(2, name="system_program", desc="System program")]
    #[account(3, name="rent", desc="Rent info")]
    InitializeMint(InitializeMintArgs),
    /// Initializes a new account to hold tokens.  If this account is associated
    /// with the native mint then the token balance of the initialized account
    /// will be equal to the amount of PUT in the account. If this account is
    /// associated with another mint, that mint must be initialized before this
    /// command can succeed.
    ///
    /// The `InitializeAccount` instruction requires no signers and MUST be
    /// included within the same Transaction as the system program's
    /// `CreateAccount` instruction that creates the account being initialized.
    /// Otherwise another party can acquire ownership of the uninitialized
    /// account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]`  The NFT account
    ///   1. `[writable]`  The mint this account will be associated with.
    ///   2. `[]` The new account's owner.
    ///   3. `[]` the system_program
    ///   4. `[]` Rent sysvar
    #[account(0, writable, name="nft_pubkey", desc="nft key")]
    #[account(1, writable, name="mint", desc="Mint key")]
    #[account(2, signer, name="owner", desc="the nft owner key")]
    #[account(3, name="system_program", desc="System program")]
    #[account(4, name="rent", desc="Rent info")]
    MintTo {
        /// The uri of the nft
        uri : String,
    },
    /// Transfers tokens from one account to another either directly or via a
    /// delegate.  If this account is associated with the native mint then equal
    /// amounts of PUT and Tokens will be transferred to the destination
    /// account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   * Single owner/delegate
    ///   0. `[writable]` The source account.
    ///   1. `[writable]` The destination account.
    ///   2. `[signer]` The source account's owner/delegate.
    ///
    ///   * Multisignature owner/delegate
    ///   0. `[writable]` The source account.
    ///   1. `[writable]` The destination account.
    ///   2. `[]` The source account's multisignature owner/delegate.
    ///   3. ..3+M `[signer]` M signer accounts.
    #[account(0, signer, name="from", desc="the nft old owner")]
    #[account(1, name="to", desc="the nft new owner")]
    #[account(2, writable, name="nft_pubkey", desc="the nft key")]
    Transfer,

    /// update url of nft
    #[account(0, writable, name="address_pubkey", desc="the address that will be update")]
    #[account(1, signer, name="owner", desc="the address's owner")]
    Update (UpdateType),

    /// Freeze a nft
    #[account(0, writable, name="nft_account", desc="the nft that will be frozen")]
    #[account(1, signer, name="authority_account", desc="the mint freeze authority")]
    #[account(2, signer, name="mint_account", desc="the mint of nft")]
    Freeze,

    /// Thaw a nft
    #[account(0, writable, name="nft_account", desc="the nft that will be thaw")]
    #[account(1, signer, name="authority_account", desc="the mint freeze authority")]
    #[account(2, signer, name="mint_account", desc="the mint of nft")]
    Thaw,

    /// set authority
    #[account(0, name="authorize_account", desc="the account that authority will be set")]
    #[account(1, signer, name="owner_account", desc="the authorize_account owner account")]
    SetAuthority(SetAuthorityArgs),

    /// burn a nft
    #[account(0, writable, name="nft_account", desc="the nft that will be burn")]
    #[account(1, signer, name="authority_account", desc="the nft owner account")]
    Burn,

}

/// SetAuthorityArgs
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SetAuthorityArgs {
    /// The type of authority to update.
    pub authority_type: AuthorityType,
    /// The new authority
    pub new_authority: Option<Pubkey>,
}


/// Nft update type
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum UpdateType {
    /// update the mint icon
    Icon{
        /// the mint icon_uri
        icon_uri: String
    },
    /// update the nft token_uri
    NftAsset{
        ///the nft token_uri
        token_uri: String
    }
}

impl TokenInstruction {
    /// deserialize
    pub fn deserialize(buf: &[u8]) -> std::io::Result<Self> {
        TokenInstruction::try_from_slice(buf)
    }
    /// serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(self).unwrap()
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
/// the instruction of InitializeMint's args
pub struct InitializeMintArgs {
    /// Number of base 10 digits to the right of the decimal place.
    pub total_supply: u64,
    /// The authority/multisignature to mint tokens.
    pub mint_authority: Pubkey,
    /// The freeze authority/multisignature of the mint.
    pub freeze_authority: Option<Pubkey>,
    /// The name of the mint
    pub name: String,
    /// The symbol of the mint
    pub symbol: String,
    /// The icon uri of the mint
    pub icon_uri: String
}

/// Creates a `InitializeMint` instruction.
pub fn initialize_mint(
    token_program_id: Pubkey,
    mint_pubkey: Pubkey,
    mint_authority_pubkey: Pubkey,
    freeze_authority_pubkey: Option<Pubkey>,
    total_supply: u64,
    name: String,
    symbol: String,
    icon_uri: String
) -> Result<Instruction, ProgramError> {
    check_program_account(&token_program_id)?;

    let mint_args = InitializeMintArgs{
        total_supply,
        mint_authority: mint_authority_pubkey,
        freeze_authority: freeze_authority_pubkey,
        name,
        symbol,
        icon_uri
    };
    let init_ins = TokenInstruction::InitializeMint(mint_args);
    let ins_data = init_ins.serialize();

    let accounts = vec![
        AccountMeta::new(mint_pubkey, true),
        AccountMeta::new(mint_authority_pubkey, true),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false)
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `MintTo` instruction.
pub fn create_mint_to_inst(
    nft_account_pubkey: Pubkey,
    mint_pubkey: Pubkey,
    owner_pubkey: Pubkey,
    token_program_id: Pubkey,
    token_uri: String
) -> Result<Instruction, ProgramError> {
    check_program_account(&token_program_id)?;

    let init_ins = TokenInstruction::MintTo {uri: token_uri};
    let ins_data = init_ins.serialize();

    let accounts = vec![
        AccountMeta::new(nft_account_pubkey, false),
        AccountMeta::new(mint_pubkey, false),
        AccountMeta::new(owner_pubkey, true),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false)
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `Transfer` instruction.
pub fn create_transfer_inst(
    from_pubkey: Pubkey,
    to_pubkey: Pubkey,
    nft_account_pubkey: Pubkey,
    token_program_id: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&token_program_id)?;

    let init_ins = TokenInstruction::Transfer;
    let ins_data = init_ins.serialize();

    let accounts = vec![
        AccountMeta::new(from_pubkey, true),
        AccountMeta::new(to_pubkey, false),
        AccountMeta::new(nft_account_pubkey, false),
        // AccountMeta::new_readonly(put_program::system_program::id(), false),
        // AccountMeta::new_readonly(sysvar::rent::id(), false)
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `Update` instruction.
pub fn update_instruction(
    address_account: Pubkey,
    owner_account: Pubkey,
    update_type: UpdateType,
    token_program_id: Pubkey
) -> Result<Instruction, ProgramError> {

    let update_ins = TokenInstruction::Update(update_type);
    let ins_data = update_ins.serialize();

    let accounts = vec![
        AccountMeta::new(address_account, false),
        AccountMeta::new(owner_account, true),
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `Freeze` instruction.
pub fn create_freeze_instruction(
    nft_account: Pubkey,
    authority_account: Pubkey,
    mint_account: Pubkey,
    token_program_id: Pubkey
) -> Result<Instruction, ProgramError> {

    let freeze_ins = TokenInstruction::Freeze;
    let ins_data = freeze_ins.serialize();

    let accounts = vec![
        AccountMeta::new(nft_account, false),
        AccountMeta::new(authority_account, true),
        AccountMeta::new(mint_account, false),
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `Thaw` instruction.
pub fn create_thaw_instruction(
    nft_account: Pubkey,
    authority_account: Pubkey,
    mint_account: Pubkey,
    token_program_id: Pubkey
) -> Result<Instruction, ProgramError> {

    let thaw_ins = TokenInstruction::Thaw;
    let ins_data = thaw_ins.serialize();

    let accounts = vec![
        AccountMeta::new(nft_account, false),
        AccountMeta::new(authority_account, true),
        AccountMeta::new(mint_account, false),
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `Burn` instruction.
pub fn create_burn_instruction(
    nft_account: Pubkey,
    authority_account: Pubkey,
    token_program_id: Pubkey
) -> Result<Instruction, ProgramError> {

    let burn_ins = TokenInstruction::Burn;
    let ins_data = burn_ins.serialize();

    let accounts = vec![
        AccountMeta::new(nft_account, false),
        AccountMeta::new(authority_account, true),
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates a `Burn` instruction.
pub fn create_authorize_instruction(
    authorize_account: Pubkey,
    new_authority: Option<Pubkey>,
    authority_type: AuthorityType,
    owner_account: Pubkey,
    token_program_id: Pubkey
) -> Result<Instruction, ProgramError> {

    let set_authority_args = SetAuthorityArgs {
        authority_type,
        new_authority
    };
    let authorize_ins = TokenInstruction::SetAuthority (set_authority_args);
    let ins_data = authorize_ins.serialize();

    let accounts = vec![
        AccountMeta::new(authorize_account, false),
        AccountMeta::new(owner_account, true),
    ];
    Ok(Instruction {
        program_id: token_program_id,
        accounts,
        data: ins_data,
    })
}


#[cfg(test)]
mod tests {
    use crate::instruction::{InitializeMintArgs, TokenInstruction};

    #[test]
    fn test_init_mint_instruction_covert() {
        let mint_args = InitializeMintArgs{
            total_supply: 1000,
            mint_authority: Default::default(),
            freeze_authority: None,
            name: "terri".to_string(),
            symbol: "terri sym".to_string(),
            icon_uri: "www.baidu.com".to_string()
        };
        let init_ins = TokenInstruction::InitializeMint(mint_args);
        let ret = init_ins.serialize();
        let init_ins_data = ret.as_slice();
        println!("the se ret is {:?}", init_ins_data);

        let ins = TokenInstruction::deserialize(init_ins_data).unwrap();
        println!("the de ret is {:?}", ins);
    }
}


/// Specifies the authority type for SetAuthority instructions
#[repr(u8)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum AuthorityType {
    /// Authority to mint new tokens
    MintTokens,
    /// Authority to freeze any account associated with the Mint
    FreezeAccount,
    /// Authority to close a token account
    CloseAccount,
}