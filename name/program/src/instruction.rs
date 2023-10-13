use borsh::{BorshDeserialize, BorshSerialize };
use put_program::instruction::{AccountMeta, Instruction};
use put_program::program_error::ProgramError;
use put_program::pubkey::Pubkey;
use put_program::{system_program, sysvar};
use ppl_sig::utils::find_proposal_account;
use crate::{check_program_account, multi_sig_account_inline, oracle_program, usdt_token_account};

/// Instructions supported by the token program.
// #[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum NameInstruction {

    /// Create top domain
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain account
    ///   2. `[]` the payer account
    ///   3. '[]' the system account
    ///   4. '[]' the rent account
    ///
    CreateTopDomain{
        /// The domain name. for example .put
        domain_name: String,
        /// Charging rules
        rule: [u128; 5],
        /// The largest space for domain resolve account.
        max_space: u16,
    },
    /// Create domain
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain account
    ///   2. `[signer]` the owner account
    ///   3. `[]` the parent account
    ///   4. `[signer]` the payer account
    ///   5. `[]` the rent account
    ///   6. `[]` the system program account
    ///

    CreateDomain {
        /// Domain name, for example: aaaa.put
        domain_name: String,
    },

    /// Create rare domain, rare domain less and equal 3
    /// characters domain, first domain account
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain account
    ///   2. `[signer]` the owner account
    ///   3. `[]` the parent account
    ///   4. `[signer]` the payer account
    ///   5. `[]` the rent account
    ///   6. `[]` the system program account
    ///

    CreateRareDomain {
        /// Rare Domain name, eg: aaaa.put.
        domain_name: String,
    },

    /// Create DomainResolveAccount.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain resolve account
    ///   2. `[signer]` the parent owner account
    ///   3. `[]` the parent account
    ///   4. `[signer]` the payer account
    ///   5. `[]` the address resolve account
    ///
    CreateDomainResolveAccount {
        /// Resolve Domain name.  For example: aaaa.put or bbb.aaaa.put or ccc.bbb.aaaa.put
        domain_name: String,

        /// The value of the domain resolve account, for .put it's put account address.
        value: Vec<u8>,
    },
    /// Transfer the Domain account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain account
    ///   2. `[signer]` the domain owner account
    ///   3. `[]` the receipt account
    ///
    Transfer,
    /// Update DomainResolveAccount
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain resolve account
    ///   2. `[signer]` the parent owner account
    ///   3. `[]` the parent account
    ///   4. '[]' old address account
    ///   5. '[]' new address account
    ///   6. `[signer]` the payer account
    ///   7. `[]` the system program account
    ///   8. `[]` the rent account
    ///
    UpdateDomainResolveAccount {
        /// address value, for .put its put account address
        new_value: Vec<u8>,
        /// Resolve account name.
        domain_name: String
    },

    /// Close DomainResolveAccount
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain resolve account
    ///   2. `[signer]` the parent owner account
    ///   3. `[]` the parent account
    ///   4. '[]' the address resolve account
    ///
    CloseDomainResolveAccount,


    /// Unbind AddressAccount
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain resolve account
    ///   2. `[signer]` the parent owner account
    ///   3. `[]` the parent account
    ///   4. '[]' the address resolve account
    ///
    UnbindAddressAccount,

    /// Renewal domain
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the domain account
    ///   2. `[]` the parent account
    ///   3. `[signer]` the payer account
    ///   4. `[]` the system program account
    ///
    Renewal,

    /// Set Top domain account's Receipt account
    ///
    /// Accounts expected by this instruction:
    ///
    ///   1. `[]` the top domain account
    ///
    SetTopReceipt {
        /// New receipt account.
        new_receipt_account: Pubkey
    },
}

impl NameInstruction {
    /// deserialize
    pub fn deserialize(buf: &[u8]) -> std::io::Result<Self> {
        NameInstruction::try_from_slice(buf)
    }
    /// serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(self).unwrap()
    }
}

// pub fn valid_domain(input: &str) -> Option<&str> {
//     lazy_static! {
//         static ref RE: Regex = Regex::new(r"^[a-z]*(\.[a-z])*(\.[a-z]*)$").unwrap();
//     }
//     if RE.is_match(input) {
//         Some(input)
//     } else {
//         None
//     }
// }

/// To determine whether effective domain name format.
pub fn is_valid_domain(input: &str) -> bool {
    for c in input.chars() {
        if !is_lower_alph_char(c) && !c.is_numeric() && c !='.' {
            return false
        }
    }
    let input_split = input.split(".").collect::<Vec<&str>>();
    let input_split_len = input_split.len();
    if input_split_len< 2 || input_split_len > 4 {
        return false
    }
    for (index, child_domain) in input_split.iter().enumerate() {
        if child_domain.len() > 32 {
            return false
        }
        // .a.put、..put、. 、put. invalid, but .put is valid
        if child_domain.len() == 0 && input_split_len != 2{
            return false
        }
        // top domain must include alphabet
        if index == input_split_len - 1  && !is_lower_alph(child_domain) {
            return false
        }
    }

    true
}

/// Determine whether a string to lowercase letters.
fn is_lower_alph(input :&str) -> bool {
    if input.len() == 0 {
        return false
    }
    for c in input.chars() {
        if !c.is_alphabetic() && !c.is_lowercase() {
            return false
        }
    }
    true
}

/// To determine whether a char is lowercase letters.
fn is_lower_alph_char(input: char) -> bool {
    input.is_alphabetic() && input.is_lowercase()
}

/// To obtain the domain name corresponding to the top-level domain name,
/// a valid domain name format needs to be passed in.
pub fn get_top_domain(domain: &String) -> String {
    let domain_split =  domain.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    let top_domain = ".".to_string() + domain_split.get(domain_len - 1).unwrap();
    top_domain
}

/// Obtain the first-level domain name corresponding to the domain name.
pub fn get_domain(domain: &String) -> String {
    let domain_split =  domain.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();
    let domain = domain_split.get(domain_len - 2).unwrap().to_string() + "." + domain_split.get(domain_len - 1).unwrap();
    domain
}

/// Creates `CreateTopDomain` instruction.
pub fn create_top_domain(
    name_program_id: Pubkey,
    domain_name: String,
    rule: [u128; 5],
    max_space: u16,
    proposal_account_puk: Pubkey,
    domain_account: Pubkey,
    payer_account: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let ctd_ins = NameInstruction::CreateTopDomain{domain_name: domain_name.clone(), rule, max_space};
    let ins_data = ctd_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_account, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new_readonly(multi_sig_account_inline::id(), false),
        AccountMeta::new(proposal_account_puk, false),
        AccountMeta::new_readonly(ppl_sig::id(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `CreateDomain` instruction.
pub fn create_domain(
    name_program_id: Pubkey,
    domain_name: String,
    domain_account: Pubkey,
    owner_account: Pubkey,
    parent_account: Pubkey,
    payer_account: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let cd_ins = NameInstruction::CreateDomain{domain_name};
    let ins_data = cd_ins.serialize();

    let oracle_seeds = &["price".as_bytes(), &system_program::id().to_bytes(), &usdt_token_account::id().to_bytes()];
    let (oracle_account_puk, _) = Pubkey::find_program_address(oracle_seeds, &oracle_program::id());
    println!("oracle address {}", oracle_account_puk);
    let accounts = vec![
        AccountMeta::new(domain_account, false),
        AccountMeta::new(owner_account, true),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new_readonly(oracle_account_puk, false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `CreateRareDomain` instruction.
pub fn create_rare_domain(
    name_program_id: Pubkey,
    domain_name: String,
    proposal_account_puk: Pubkey,
    domain_account: Pubkey,
    owner_account: Pubkey,
    parent_account: Pubkey,
    payer_account: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let crd_ins = NameInstruction::CreateRareDomain{domain_name: domain_name.clone()};
    let ins_data = crd_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_account, false),
        AccountMeta::new(owner_account, false),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new_readonly(multi_sig_account_inline::id(), false),
        AccountMeta::new(proposal_account_puk, false),
        AccountMeta::new_readonly(ppl_sig::id(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `CreateDomainResolveAccount` instruction.
pub fn create_domain_resolve_account(
    name_program_id: Pubkey,
    domain_name: String,
    domain_account: Pubkey,
    parent_owner_account: Pubkey,
    parent_account: Pubkey,
    payer_account: Pubkey,
    address_resolve_account: Pubkey,
    value_data: Vec<u8>
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let cdra_ins = NameInstruction::CreateDomainResolveAccount { domain_name, value: value_data };
    let ins_data = cdra_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_account, false),
        AccountMeta::new(parent_owner_account, true),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new(address_resolve_account, false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `Transfer` instruction.
pub fn create_transfer(
    name_program_id: Pubkey,
    domain_account: Pubkey,
    domain_owner_account: Pubkey,
    receipt: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let transfer_ins = NameInstruction::Transfer;
    let ins_data = transfer_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_account, false),
        AccountMeta::new(domain_owner_account, true),
        AccountMeta::new(receipt, false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `UpdateDomainResolveAccount` instruction.
pub fn update_domain_resolve_account(
    name_program_id: Pubkey,
    domain_name: String,
    new_value_data: Vec<u8>,
    domain_resolve_account: Pubkey,
    parent_owner_account: Pubkey,
    parent_account: Pubkey,
    old_address: Pubkey,
    new_address: Pubkey,
    top_domain: Pubkey,
    payer_account: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let udra_ins = NameInstruction::UpdateDomainResolveAccount{ new_value: new_value_data, domain_name };
    let ins_data = udra_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_resolve_account, false),
        AccountMeta::new(parent_owner_account, true),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(old_address, false),
        AccountMeta::new(new_address, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new(top_domain, false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `CloseDomainResolveAccount` instruction.
pub fn close_domain_resolve_account(
    name_program_id: Pubkey,
    domain_resolve_account: Pubkey,
    parent_owner_account: Pubkey,
    parent_account: Pubkey,
    address_resolve_account: Pubkey,
    top_domain: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let cdra_ins = NameInstruction::CloseDomainResolveAccount;
    let ins_data = cdra_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_resolve_account, false),
        AccountMeta::new(parent_owner_account, true),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(address_resolve_account, false),
        AccountMeta::new(top_domain, false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `UnbindAddressAccount` instruction.
pub fn unbind_address_account(
    name_program_id: Pubkey,
    domain_resolve_account: Pubkey,
    parent_owner_account: Pubkey,
    parent_account: Pubkey,
    address_resolve_account: Pubkey,
    top_domain: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let uaa_ins = NameInstruction::UnbindAddressAccount;
    let ins_data = uaa_ins.serialize();

    let accounts = vec![
        AccountMeta::new(domain_resolve_account, false),
        AccountMeta::new(parent_owner_account, true),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(address_resolve_account, false),
        AccountMeta::new(top_domain, false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `Renewal` instruction.
pub fn create_renewal(
    name_program_id: Pubkey,
    domain_account: Pubkey,
    parent_account: Pubkey,
    payer_account: Pubkey,
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let renewal_ins = NameInstruction::Renewal;
    let ins_data = renewal_ins.serialize();

    let oracle_seeds = &["price".as_bytes(), &system_program::id().to_bytes(), &usdt_token_account::id().to_bytes()];
    let (oracle_account_puk, _) = Pubkey::find_program_address(oracle_seeds, &oracle_program::id());

    let accounts = vec![
        AccountMeta::new(domain_account, false),
        AccountMeta::new(parent_account, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new_readonly(oracle_account_puk, false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
    ];
    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

/// Creates `SetTopReceipt` instruction.
pub fn set_top_receipt(
    name_program_id: Pubkey,
    top_domain_account: Pubkey,
    receipt: Pubkey,
    payer_account: Pubkey,
    nonce: u64
) -> Result<Instruction, ProgramError> {
    check_program_account(&name_program_id)?;

    let renewal_ins = NameInstruction::SetTopReceipt { new_receipt_account: receipt.clone() };
    let ins_data = renewal_ins.serialize();

    let (proposal_account_puk, _) = find_proposal_account(&payer_account, nonce);
    println!("creating ppl-sig proposal account {}", proposal_account_puk);

    let accounts = vec![
        AccountMeta::new(top_domain_account, false),
        AccountMeta::new(payer_account, true),
        AccountMeta::new_readonly(multi_sig_account_inline::id(), false),
        AccountMeta::new(proposal_account_puk, false),
        AccountMeta::new_readonly(ppl_sig::id(), false),
        AccountMeta::new_readonly(put_program::system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];

    Ok(Instruction {
        program_id: name_program_id,
        accounts,
        data: ins_data,
    })
}

#[cfg(test)]
mod tests {
    use crate::instruction::is_valid_domain;

    #[test]
    /// test_is_valid_domain
    fn test_is_valid_domain() {
        let valid_domain1 = ".put";
        let valid_domain2 = "abb.put";
        let valid_domain3 = "ccc12.abb.put";
        let valid_domain4 = "ga.ccc12.abb.put";
        let invalid_domain1 = "AS.put";
        let invalid_domain2 = ".AS.put";
        let invalid_domain3 = "AS.put.";
        let invalid_domain4 = "AS.put.123";
        let invalid_domain5 = ".";
        let invalid_domain6 = "11.ga.ccc12.abb.put";

        assert!(is_valid_domain(valid_domain1));
        assert!(is_valid_domain(valid_domain2));
        assert!(is_valid_domain(valid_domain3));
        assert!(is_valid_domain(valid_domain4));
        assert!(!is_valid_domain(invalid_domain1));
        assert!(!is_valid_domain(invalid_domain2));
        assert!(!is_valid_domain(invalid_domain4));
        assert!(!is_valid_domain(invalid_domain5));
        assert!(!is_valid_domain(invalid_domain6));

    }
}