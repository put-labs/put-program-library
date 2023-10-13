use borsh::{ BorshDeserialize, BorshSerialize };
use put_program::instruction::{AccountMeta, Instruction};
use put_program::program_error::ProgramError;
use put_program::pubkey::Pubkey;
use put_program::sysvar;
use crate::check_program_account;

/// Instructions supported by the token program.
// #[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum SigInstruction {
    /// Create Multi-sig Account
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the Multi-sig account
    ///   2. `[]` the payer account
    ///   3. '[]' the system account
    ///   4. '[]' the rent account
    ///
    CreateMultiSigAccount{
        /// singers
        accounts: Vec<Pubkey>,
        /// threshold
        threshold: u8,
    },

    /// Create Proposal Account
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the Multi-sig account
    ///   2. '[]' the payer account
    ///   3. `[]` the proposal account
    ///   4. '[]' the system account
    ///   5. '[]' the rent account
    ///
    CreateInitProposalAccount{
        /// the multi sig account
        parent_key: Pubkey,
        /// transaction proposal summary
        summary: String,
        /// Continuous validity period,
        /// e.g. 2000 present 2000seconds
        continuous_validity_period: u32,
    },

    /// Vote, vote for a transaction
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the proposal account
    ///   2. `[]` the multi_sig account
    ///   3. '[]' the singer account
    ///
    Vote,

    /// verify, verify for a transaction can be execute
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the multi_sig account
    ///   2. `[]` the initiator account
    ///   3. `[]` the proposal account
    ///
    Verify {
        /// transaction proposal summary
        summary: Option<String>,
    },

    /// AddSigner, add new signer
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the multi_sig account
    ///   2. `[]` the admin account
    ///
    AddSigner {
        /// the new signer
        signer: Pubkey
    },

    /// RemoveSigner, remove a signer
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the multi_sig account
    ///   2. `[]` the admin account
    ///
    RemoveSigner {
        /// the signer will be remove
        signer: Pubkey
    },

    /// CloseProposalAccount, close a proposal
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the proposal initiator account
    ///   2. `[]` the proposal account
    ///
    CloseProposalAccount,

    /// CloseProposalAccount, close a proposal
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the multi_sig account
    ///   2. `[]` the initiator account
    ///   3. `[]` the proposal  account
    ///   4. `[]` the system  account
    ///   5. `[]` the rent  account
    ///
    SetMultiSigThreshold {
        /// New threshold
        new_threshold: u8
    },
}

impl SigInstruction {
    /// deserialize
    pub fn deserialize(buf: &[u8]) -> std::io::Result<Self> {
        SigInstruction::try_from_slice(buf)
    }
    /// serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(self).unwrap()
    }
}

/// Creates a `CreateMultiSigAccount` instruction.
pub fn create_multi_sig_account(
    sig_program_id: &Pubkey,
    sig_accounts: Vec<Pubkey>,
    threshold: u8,
    multi_sig_account: &Pubkey,
    payer_account: &Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let cmsa_ins = SigInstruction::CreateMultiSigAccount{ accounts: sig_accounts, threshold };
    let ins_data = cmsa_ins.serialize();

    let accounts = vec![
        AccountMeta::new(multi_sig_account.clone(), true),
        AccountMeta::new(payer_account.clone(), true),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `CreateProposalAccount` instruction.
pub fn create_proposal_account(
    sig_program_id: &Pubkey,
    summary: String,
    validity_period: u32,
    multi_sig_account: &Pubkey,
    initiator_account: &Pubkey,
    proposal_account: &Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let cipa_ins = SigInstruction::CreateInitProposalAccount{
        parent_key: multi_sig_account.clone(),
        summary,
        continuous_validity_period: validity_period
    };
    let ins_data = cipa_ins.serialize();

    let accounts = vec![
        AccountMeta::new(multi_sig_account.clone(), false),
        AccountMeta::new(initiator_account.clone(), true),
        AccountMeta::new(proposal_account.clone(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `CreateProposalAccount` instruction.
pub fn create_close_proposal_ins(
    sig_program_id: &Pubkey,
    payer_account: &Pubkey,
    proposal_account: &Pubkey,
    multi_sig_account: &Pubkey
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let cpa_ins = SigInstruction::CloseProposalAccount;
    let ins_data = cpa_ins.serialize();

    let accounts = vec![
        AccountMeta::new(payer_account.clone(), true),
        AccountMeta::new(proposal_account.clone(), false),
        AccountMeta::new(multi_sig_account.clone(), false),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `Vote` instruction.
pub fn create_vote_ins(
    sig_program_id: &Pubkey,
    multi_sig_account: &Pubkey,
    proposal_account: &Pubkey,
    signer_account: &Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let cv_ins = SigInstruction::Vote;
    let ins_data = cv_ins.serialize();

    let accounts = vec![
        AccountMeta::new(multi_sig_account.clone(), false),
        AccountMeta::new(proposal_account.clone(), false),
        AccountMeta::new(signer_account.clone(), true),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `verify` instruction.
pub fn verify(
    sig_program_id: &Pubkey,
    multi_sig_account: &Pubkey,
    initiator_account: &Pubkey,
    proposal_account: &Pubkey,
    summary: Option<String>,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let cv_ins = SigInstruction::Verify{ summary };
    let ins_data = cv_ins.serialize();

    let accounts = vec![
        AccountMeta::new_readonly(multi_sig_account.clone(), false),
        AccountMeta::new(initiator_account.clone(), true),
        AccountMeta::new(proposal_account.clone(), false),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `AddSigner` instruction.
pub fn create_add_signer(
    sig_program_id: &Pubkey,
    multi_sig_account: &Pubkey,
    initiator_account: &Pubkey,
    proposal_account: &Pubkey,
    signer: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let as_ins = SigInstruction::AddSigner { signer };
    let ins_data = as_ins.serialize();

    println!("creating proposal account {}", proposal_account);

    let accounts = vec![
        AccountMeta::new(multi_sig_account.clone(), false),
        AccountMeta::new(initiator_account.clone(), true),
        AccountMeta::new(proposal_account.clone(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];

    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `RemoveSigner` instruction.
pub fn create_remove_signer(
    sig_program_id: &Pubkey,
    multi_sig_account: &Pubkey,
    initiator_account: &Pubkey,
    proposal_account: &Pubkey,
    signer: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let rs_ins = SigInstruction::RemoveSigner { signer };
    let ins_data = rs_ins.serialize();

    println!("creating proposal account {}", proposal_account);

    let accounts = vec![
        AccountMeta::new(multi_sig_account.clone(), false),
        AccountMeta::new(initiator_account.clone(), true),
        AccountMeta::new(proposal_account.clone(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}

/// Creates a `SetMultiSigThreshold` instruction.
pub fn create_set_new_threshold(
    sig_program_id: &Pubkey,
    multi_sig_account: &Pubkey,
    initiator_account: &Pubkey,
    proposal_account: &Pubkey,
    new_threshold: u8,
) -> Result<Instruction, ProgramError> {
    check_program_account(&sig_program_id)?;

    let sst_ins = SigInstruction::SetMultiSigThreshold { new_threshold };
    let ins_data = sst_ins.serialize();

    println!("creating proposal account {}", proposal_account);

    let accounts = vec![
        AccountMeta::new(multi_sig_account.clone(), false),
        AccountMeta::new(initiator_account.clone(), true),
        AccountMeta::new(proposal_account.clone(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: sig_program_id.clone(),
        accounts,
        data: ins_data,
    })
}