#![allow(deprecated)] // TODO: Remove when SPL upgrades to put 1.8
use clap::{crate_description, crate_name, crate_version, value_t_or_exit, App, AppSettings, Arg, ArgMatches, SubCommand, value_t};
use serde::Serialize;

use put_clap_utils::{
    fee_payer::fee_payer_arg,
    input_parsers::{pubkey_of, pubkey_of_signer},
    input_validators::{
        is_url_or_moniker, is_valid_pubkey,
        is_valid_signer, normalize_to_url_if_moniker,
    },
    keypair::{signer_from_path, CliSignerInfo},
    nonce::*,
    ArgConstant, DisplayError,
};
use put_cli_output::{
    return_signers_data, CliSignOnlyData, CliSignature, OutputFormat, QuietDisplay,
    ReturnSignersConfig, VerboseDisplay,
};
use put_client::{
    blockhash_query::BlockhashQuery, rpc_client::RpcClient,
};
use put_remote_wallet::remote_wallet::RemoteWalletManager;
use put_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    message::Message,
    native_token::*,
    pubkey::Pubkey,
    signature::{Signer},
    transaction::Transaction,
};
// use spl_associated_token_account::{
//     get_associated_token_address_with_program_id, instruction::create_associated_token_account,
// };


use std::{fmt::Display, process::exit, str::FromStr, sync::Arc};
use lazy_static::lazy_static;
use put_sdk::hash::hashv;
use regex::Regex;
use borsh::BorshDeserialize;
use put_account_decoder::parse_name::NameAccountType;
use put_account_decoder::UiAccountData;
use put_clap_utils::offline::{BLOCKHASH_ARG, DUMP_TRANSACTION_MESSAGE, SIGN_ONLY_ARG};

mod config;
use config::Config;

mod output;
use output::*;
use ppl_name::instruction::{close_domain_resolve_account, create_domain, create_domain_resolve_account, create_rare_domain, create_renewal, create_top_domain, create_transfer, get_domain, is_valid_domain, set_top_receipt, unbind_address_account, update_domain_resolve_account};
use ppl_name::multi_sig_account_inline;
use ppl_name::state::{AccountType, DomainResolveAccount, get_seeds_and_key, TopDomainAccount};
use ppl_sig::state::MultiSigAccount;
use ppl_sig::utils::find_proposal_account;


pub const OWNER_ADDRESS_ARG: ArgConstant<'static> = ArgConstant {
    name: "owner",
    long: "owner",
    help: "Address of the token's owner. Defaults to the client keypair address.",
};

pub const OWNER_KEYPAIR_ARG: ArgConstant<'static> = ArgConstant {
    name: "owner",
    long: "owner",
    help: "Keypair of the token's owner. Defaults to the client keypair.",
};

pub const MINT_ADDRESS_ARG: ArgConstant<'static> = ArgConstant {
    name: "mint_address",
    long: "mint-address",
    help: "Address of mint that token account is associated with. Required by --sign-only",
};

pub const MINT_DECIMALS_ARG: ArgConstant<'static> = ArgConstant {
    name: "mint_decimals",
    long: "mint-decimals",
    help: "Decimals of mint that token account is associated with. Required by --sign-only",
};

pub const DELEGATE_ADDRESS_ARG: ArgConstant<'static> = ArgConstant {
    name: "delegate_address",
    long: "delegate-address",
    help: "Address of delegate currently assigned to token account. Required by --sign-only",
};

pub const MULTISIG_SIGNER_ARG: ArgConstant<'static> = ArgConstant {
    name: "multisig_signer",
    long: "multisig-signer",
    help: "Member signer of a multisig account",
};

pub const CREATE_TOKEN: &str = "create-token";

pub fn owner_address_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name(OWNER_ADDRESS_ARG.name)
        .long(OWNER_ADDRESS_ARG.long)
        .takes_value(true)
        .value_name("OWNER_ADDRESS")
        .validator(is_valid_pubkey)
        .help(OWNER_ADDRESS_ARG.help)
}

pub fn owner_keypair_arg_with_value_name<'a, 'b>(value_name: &'static str) -> Arg<'a, 'b> {
    Arg::with_name(OWNER_KEYPAIR_ARG.name)
        .long(OWNER_KEYPAIR_ARG.long)
        .takes_value(true)
        .value_name(value_name)
        .validator(is_valid_signer)
        .help(OWNER_KEYPAIR_ARG.help)
}

pub fn owner_keypair_arg<'a, 'b>() -> Arg<'a, 'b> {
    owner_keypair_arg_with_value_name("OWNER_KEYPAIR")
}

pub fn mint_address_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name(MINT_ADDRESS_ARG.name)
        .long(MINT_ADDRESS_ARG.long)
        .takes_value(true)
        .value_name("MINT_ADDRESS")
        .validator(is_valid_pubkey)
        .requires(SIGN_ONLY_ARG.name)
        .requires(BLOCKHASH_ARG.name)
        .help(MINT_ADDRESS_ARG.help)
}


pub(crate) type Error = Box<dyn std::error::Error>;

pub(crate) type CommandResult = Result<String, Error>;

fn get_signer(
    matches: &ArgMatches<'_>,
    keypair_name: &str,
    wallet_manager: &mut Option<Arc<RemoteWalletManager>>,
) -> Option<(Box<dyn Signer>, Pubkey)> {
    matches.value_of(keypair_name).map(|path| {
        let signer =
            signer_from_path(matches, path, keypair_name, wallet_manager).unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                exit(1);
            });
        let signer_pubkey = signer.pubkey();
        (signer, signer_pubkey)
    })
}

pub(crate) fn check_fee_payer_balance(config: &Config, required_balance: u128) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.fee_payer)?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            config.fee_payer,
            lamports_to_put(required_balance),
            lamports_to_put(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

type SignersOf = Vec<(Box<dyn Signer>, Pubkey)>;
pub fn signers_of(
    matches: &ArgMatches<'_>,
    name: &str,
    wallet_manager: &mut Option<Arc<RemoteWalletManager>>,
) -> Result<Option<SignersOf>, Box<dyn std::error::Error>> {
    if let Some(values) = matches.values_of(name) {
        let mut results = Vec::new();
        for (i, value) in values.enumerate() {
            let name = format!("{}-{}", name, i + 1);
            let signer = signer_from_path(matches, value, &name, wallet_manager)?;
            let signer_pubkey = signer.pubkey();
            results.push((signer, signer_pubkey));
        }
        Ok(Some(results))
    } else {
        Ok(None)
    }
}

fn command_domains(config: &Config, owner: Pubkey) -> CommandResult {
    println!("find domains by owner, owner {}", owner.to_string());
    let accounts = config.rpc_client.get_domain_accounts_by_owner(
        &owner,
    )?;
    if accounts.is_empty() {
        println!("None");
        return Ok("".to_string());
    }
    println!("-------------------------------------------------------------------------");
    println!("{:<44}  {:<13}  {:<8}", "Account", "AccountType", "Name");
    for account in accounts {
        if let UiAccountData::Json(account_data) = account.account.data {
            let name_account_type: NameAccountType =
                serde_json::from_value(account_data.parsed)?;

            match name_account_type {
                NameAccountType::Domain(domain) => {
                    println!("{:<44}  {:<13}  {:<8}", account.pubkey, domain.account_type, domain.domain_name)
                }
                NameAccountType::DomainResolve(resolve) => {
                    println!("{:<44}  {:<13}  {:<8}", account.pubkey, resolve.account_type, resolve.domain_name)
                }
                _ => unreachable!()
            }
        }
    }

    Ok("".to_string())
}

fn command_address_info(config: &Config, value: Vec<u8>) -> CommandResult {
    let (address_resolve_pk, _) = get_seeds_and_key(&config.program_id, None, AccountType::AddressResolve, Some(value)).unwrap();

    println!("find address {} account info ", address_resolve_pk.to_string());
    let account = config.rpc_client.get_domain_address_account(
        &address_resolve_pk,
    )?;

    println!();
    println!("Account type: {:?}", account.account_type);
    println!("Account state: {:?}", account.account_state);
    println!("Mapped Domain: {:?}", account.domain);

    Ok("".to_string())
}

fn command_account(config: &Config, domain_name: String, account_type: String) -> CommandResult {
    let domain_name = domain_name.to_lowercase();
    // Get parent domain
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    match account_type.as_str() {
        "top" => {
            if domain_len != 2 {
                return Err(format!("invalid domain format. invalid domain length.").into());
            }
            let top_domain_hash = hashv(&[domain_name.as_bytes()]);
            let (top_domain_puk, _) = get_seeds_and_key(
                &config.program_id,
                Some(top_domain_hash.to_bytes().to_vec()),
                AccountType::TopDomain, None
            ).unwrap();
            let account = config.rpc_client.get_account(
                &top_domain_puk,
            )?;
            let top_domain_data_obj : TopDomainAccount = TopDomainAccount::deserialize(&mut account.data.as_slice()).unwrap();

            println!();
            println!("Account type: {:?}", top_domain_data_obj.account_type);
            println!("Account state: {:?}", top_domain_data_obj.account_state);
            println!("Fee rule: {:?}", top_domain_data_obj.rule);
            println!("Multi sig account: {:?}", multi_sig_account_inline::id());
            println!("Top account: {}", top_domain_puk);

            Ok("".to_string())
        }
        "domain" => {
            if domain_len != 2 {
                return Err(format!("invalid domain format. invalid domain length.").into());
            }
            let domain_hash = hashv(&[domain_name.as_bytes()]);
            let (domain_account_puk, _) = get_seeds_and_key(
                &config.program_id,
                Some(domain_hash.to_bytes().to_vec()),
                AccountType::Domain, None
            ).unwrap();
            let account = config.rpc_client.get_domain_account(
                &domain_account_puk,
            )?;

            println!();
            println!("Account type: {:?}", account.account_type);
            println!("Account state: {:?}", account.account_state);
            println!("Parent key: {:?}", account.parent_key);
            println!("Owner: {:?}", account.owner);
            println!("Expire Date: {:?}", account.expire_time);
            println!("Domain account: {}", domain_account_puk);

            Ok("".to_string())
        }
        "resolve" => {
            if domain_len < 2 || domain_len > 4 {
                return Err(format!("invalid domain format. invalid domain length.").into());
            }

            // Get first level domain
            let parent_domain = get_domain(&domain_name);

            let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
            let (domain_account, _) = get_seeds_and_key(
                &config.program_id,
                Some(parent_domain_hash.to_bytes().to_vec()),
                AccountType::Domain, None
            ).unwrap();

            let domain_resolve_hash = hashv(&[domain_name.as_bytes()]);
            let (domain_resolve_account, _) =
                get_seeds_and_key(&config.program_id,Some(domain_resolve_hash.to_bytes().to_vec()), AccountType::DomainResolve, None).unwrap();

            let account = config.rpc_client.get_domain_resolve_account(
                &domain_account,
                &domain_resolve_account
            )?;
            println!();
            println!("Account type: {}", account.account_type);
            println!("Account state: {}", account.account_state);
            println!("Parent key: {}", account.parent_key);
            println!("Resolve account: {}", domain_resolve_account);
            println!("value: {}", account.value);

            Ok("".to_string())
        }
        _ => unreachable!()
    }
}

#[allow(clippy::too_many_arguments)]
fn command_set_top_receipt(
    config: &Config,
    domain_name: String,
    receipt: Pubkey,
    payer_account: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("set_top_receipt [{}]", receipt));

    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }


    let hash = hashv(&[domain_name.as_bytes()]);
    let (top_domain_account,_) =
        get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_inline::id()).unwrap();
    let multi_sig_data_obj : MultiSigAccount = MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).unwrap();

    let instructions = vec![
        set_top_receipt(
            config.program_id,
            top_domain_account,
            receipt,
            payer_account,
            multi_sig_data_obj.nonce + 1
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "set-receipt", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_renewal(
    config: &Config,
    domain_name: String,
    payer: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("renewal domain [{}]", domain_name));

    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    if domain_len != 2 {
        return Err(format!("invalid first domain format, length unmatched.").into());
    }

    let parent_domain = ".".to_string() + domain_split.get(1).unwrap();
    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_account,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    let instructions = vec![
        create_renewal(
            config.program_id,
            domain_account,
            parent_domain_account,
            payer,
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-top", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_close_resolve(
    config: &Config,
    domain_name: String,
    domain_owner_account: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("close a resolve domain [{}]", domain_name));

    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    if domain_len < 2 ||  domain_len > 4 {
        return Err(format!("invalid first domain format, length unmatched.").into());
    }

    // Get a first level domain
    let top_domain = domain_split.get(domain_len - 1).unwrap();
    let domain = domain_split.get(domain_len - 2).unwrap();
    let parent_domain = domain.to_string() + "." + top_domain;
    let complete_top_domain = ".".to_string() + top_domain;

    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    println!("parent domain: {}", parent_domain_account.to_string());
    let top_domain_hash = hashv(&[complete_top_domain.as_bytes()]);
    let (top_domain_pubkey, _) = get_seeds_and_key(&config.program_id,Some(top_domain_hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();


    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_resolve_pubkey,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::DomainResolve, None).unwrap();

    let domain_resolve_account = config.rpc_client.get_account(&domain_resolve_pubkey)?;
    let domain_resolve_data_obj : DomainResolveAccount= DomainResolveAccount::deserialize (&mut domain_resolve_account.data.as_slice()).unwrap();

    // Get old address, if not exist, replace with empty vec
    let mut current_value :Vec<u8> = Vec::new();
    let value =  domain_resolve_data_obj.value;
    if value.is_some() {
        current_value = value.unwrap();
    }
    let (current_address_resolve_pubkey, _) = get_seeds_and_key(&config.program_id, None, AccountType::AddressResolve, Some(current_value.clone())).unwrap();

    let instructions = vec![
        close_domain_resolve_account(
            config.program_id,
            domain_resolve_pubkey,
            domain_owner_account,
            parent_domain_account,
            current_address_resolve_pubkey,
            top_domain_pubkey,
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-top", config)
        }
    })
}


#[allow(clippy::too_many_arguments)]
fn command_unbind_value(
    config: &Config,
    domain_name: String,
    domain_owner_account: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("unbind a domain [{}] value", domain_name));

    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    if domain_len < 2 ||  domain_len > 4 {
        return Err(format!("invalid first domain format, length unmatched.").into());
    }

    // Get first domain
    let top_domain = domain_split.get(domain_len - 1).unwrap();
    let domain = domain_split.get(domain_len - 2).unwrap();
    let parent_domain = domain.to_string() + "." + top_domain;
    let complete_top_domain = ".".to_string() + top_domain;

    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    println!("parent domain: {}", parent_domain_account.to_string());
    let top_domain_hash = hashv(&[complete_top_domain.as_bytes()]);
    let (top_domain_pubkey, _) = get_seeds_and_key(&config.program_id,Some(top_domain_hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_resolve_pubkey,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::DomainResolve, None).unwrap();

    let domain_resolve_account = config.rpc_client.get_account(&domain_resolve_pubkey)?;
    let domain_resolve_data_obj : DomainResolveAccount= DomainResolveAccount::deserialize (&mut domain_resolve_account.data.as_slice()).unwrap();

    // Get old address, if not exist, replace with empty vec
    let mut current_value :Vec<u8> = Vec::new();
    let value =  domain_resolve_data_obj.value;
    if value.is_some() {
        current_value = value.unwrap();
    }
    let (current_address_resolve_pubkey, _) = get_seeds_and_key(&config.program_id, None, AccountType::AddressResolve, Some(current_value.clone())).unwrap();

    let instructions = vec![
        unbind_address_account(
            config.program_id,
            domain_resolve_pubkey,
            domain_owner_account,
            parent_domain_account,
            current_address_resolve_pubkey,
            top_domain_pubkey,
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-top", config)
        }
    })
}


#[allow(clippy::too_many_arguments)]
fn command_transfer(
    config: &Config,
    domain_name: String,
    domain_owner_account: Pubkey,
    receipt: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_account,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    println_display(config, format!("transfer a domain {} to {}", domain_account.to_string(), receipt.to_string()));
    let instructions = vec![
        create_transfer(
            config.program_id,
            domain_account,
            domain_owner_account,
            receipt,
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-top", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_update_resolve_domain(
    config: &Config,
    domain_name: String,
    payer: Pubkey,
    owner: Pubkey,
    new_value: Vec<u8>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("update a domain [{}] mapped value ", domain_name.clone()));
    let domain_name = domain_name.to_lowercase();

    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    if domain_len < 2 ||  domain_len > 4 {
        return Err(format!("invalid first domain format, length unmatched.").into());
    }
    // Get parent domain
    let top_domain = domain_split.get(domain_len - 1).unwrap();
    let domain = domain_split.get(domain_len - 2).unwrap();
    let parent_domain = domain.to_string() + "." + top_domain;
    let complete_top_domain = ".".to_string() + top_domain;

    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    println!("parent domain: {}", parent_domain_account.to_string());

    let top_domain_hash = hashv(&[complete_top_domain.as_bytes()]);
    let (top_domain_pubkey, _) = get_seeds_and_key(&config.program_id,Some(top_domain_hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_resolve_pubkey,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::DomainResolve, None).unwrap();

    let domain_resolve_account = config.rpc_client.get_account(&domain_resolve_pubkey)?;
    let domain_resolve_data_obj : DomainResolveAccount= DomainResolveAccount::deserialize (&mut domain_resolve_account.data.as_slice()).unwrap();

    // Get old address, if not exist, replace with empty vec
    let mut old_value :Vec<u8> = Vec::new();
    let value =  domain_resolve_data_obj.value;
    if value.is_some() {
        old_value = value.unwrap();
    }
    let (old_address_resolve_pubkey, _) = get_seeds_and_key(&config.program_id, None, AccountType::AddressResolve, Some(old_value.clone())).unwrap();
    let (new_address_resolve_pubkey, _) = get_seeds_and_key(&config.program_id, None, AccountType::AddressResolve, Some(new_value.clone())).unwrap();
    println!("program_id {}", config.program_id.to_string());
    let instructions = vec![
        update_domain_resolve_account(
            config.program_id,
            domain_name.clone(),
            new_value,
            domain_resolve_pubkey.clone(),
            owner,
            parent_domain_account,
            old_address_resolve_pubkey,
            new_address_resolve_pubkey,
            top_domain_pubkey,
            payer,
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            println!("domain account: {} updated", domain_resolve_pubkey.to_string());
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "update-resolve-domain", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_create_resolve_domain(
    config: &Config,
    domain_name: String,
    payer: Pubkey,
    owner: Pubkey,
    value: Vec<u8>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("creating a domain {}", domain_name.clone()));
    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    let domain_len = domain_split.len();

    if domain_len < 2 ||  domain_len > 4 {
        return Err(format!("invalid first domain format, length unmatched.").into());
    }

    // Get parent domain
    let top_domain = domain_split.get(domain_len - 1).unwrap();
    let domain = domain_split.get(domain_len - 2).unwrap();
    let parent_domain = domain.to_string() + "." + top_domain;

    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    println!("parent domain: {}", parent_domain_account.to_string());

    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_account,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::DomainResolve, None).unwrap();

    let (address_resolve_pubkey, _) = get_seeds_and_key(&config.program_id, None, AccountType::AddressResolve, Some(value.clone())).unwrap();
    println!("program_id {}", config.program_id.to_string());
    let instructions = vec![
        create_domain_resolve_account(
            config.program_id,
            domain_name.clone(),
            domain_account,
            owner,
            parent_domain_account,
            payer,
            address_resolve_pubkey,
            value
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            println!("domain name: {}", domain_name);
            println!("domain account: {}", domain_account.to_string());
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-top", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_create_rare_domain(
    config: &Config,
    domain_name: String,
    payer: Pubkey,
    owner: Pubkey,
    verify_proposal_account: Option<Pubkey>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("creating a domain {}", domain_name.clone()));
    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    if domain_split.len() != 2 {
        return Err(format!("invalid first domain format,length unmatched.").into());
    }
    let parent_domain = ".".to_string() + domain_split.get(1).unwrap();
    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_account,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();


    let proposal_account_puk : Pubkey;
    if verify_proposal_account.is_none() {
        let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_inline::id()).unwrap();
        let multi_sig_data_obj : MultiSigAccount = MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).unwrap();

        let (find_proposal_account_puk, _) = find_proposal_account(&multi_sig_account_inline::id(), multi_sig_data_obj.nonce + 1);
        proposal_account_puk = find_proposal_account_puk;
        println!("creating ppl-sig proposal account {}", proposal_account_puk);
    } else {
        proposal_account_puk = verify_proposal_account.unwrap();
    }

    println!("program_id {}", config.program_id.to_string());
    let instructions = vec![
        create_rare_domain(
            config.program_id,
            domain_name.clone(),
            proposal_account_puk,
            domain_account,
            owner,
            parent_domain_account,
            payer
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            println!("domain name: {}", domain_name);
            println!("domain account: {}", domain_account.to_string());
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-rare-domain", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_create_domain(
    config: &Config,
    domain_name: String,
    payer: Pubkey,
    owner: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("creating a domain {}", domain_name.clone()));
    let domain_name = domain_name.to_lowercase();
    if !is_valid_domain(&domain_name) {
        return Err(format!("invalid domain format.").into());
    }

    let domain_split =  domain_name.split(".").collect::<Vec<&str>>();
    if domain_split.len() != 2 {
        return Err(format!("invalid first domain format, too len.").into());
    }
    let parent_domain = ".".to_string() + domain_split.get(1).unwrap();
    let parent_domain_hash = hashv(&[parent_domain.as_bytes()]);
    let (parent_domain_account, _) = get_seeds_and_key(&config.program_id,Some(parent_domain_hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_account,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::Domain, None).unwrap();

    println!("program_id {}", config.program_id.to_string());
    let instructions = vec![
        create_domain(
            config.program_id,
            domain_name.clone(),
            domain_account,
            owner,
            parent_domain_account,
            payer,
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            println!("domain name: {}", domain_name);
            println!("domain account: {}", domain_account.to_string());
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-domain", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_create_top_domain(
    config: &Config,
    domain_name: String,
    rule: [u128; 5],
    max_space: u16,
    payer: Pubkey,
    verify_proposal_account: Option<Pubkey>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("creating a top domain {}", domain_name.clone()));
    let hash = hashv(&[domain_name.as_bytes()]);
    let (domain_account,_) = get_seeds_and_key(&config.program_id,Some(hash.to_bytes().to_vec()), AccountType::TopDomain, None).unwrap();

    println!("program_id {}", config.program_id.to_string());
    let proposal_account_puk : Pubkey;
    if verify_proposal_account.is_none() {
        let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_inline::id()).unwrap();
        let multi_sig_data_obj : MultiSigAccount = MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).unwrap();

        let (find_proposal_account_puk, _) = find_proposal_account(&multi_sig_account_inline::id(), multi_sig_data_obj.nonce + 1);
        proposal_account_puk = find_proposal_account_puk;
        println!("creating ppl-sig proposal account {}", proposal_account_puk);
    } else {
        proposal_account_puk = verify_proposal_account.unwrap();
    }
    let instructions = vec![
        create_top_domain(
            config.program_id,
            domain_name.clone(),
            rule,
            max_space,
            proposal_account_puk,
            domain_account,
            payer
        )?,
    ];

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        0,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => {
            println!("domain name: {}", domain_name);
            println!("domain account: {}", domain_account.to_string());
            cli_signature.to_string()
        },
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "create-top", config)
        }
    })
}


fn main() -> Result<(), Error> {
    // let default_decimals = &format!("{}", native_mint::DECIMALS);
    let default_program_id = ppl_name::id().to_string();
    let app_matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *put_cli_config::CONFIG_FILE {
                arg.default_value(config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .global(true)
                .help("Show additional information"),
        )
        .arg(
            Arg::with_name("output_format")
                .long("output")
                .value_name("FORMAT")
                .global(true)
                .takes_value(true)
                .possible_values(&["json", "json-compact"])
                .help("Return information in specified output format"),
        )
        .arg(
            Arg::with_name("program_id")
                .short("p")
                .long("program-id")
                .value_name("ADDRESS")
                .takes_value(true)
                .global(true)
                .default_value(&default_program_id)
                .validator(is_valid_pubkey)
                .help("PPL Nft program id"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .short("u")
                .long("url")
                .value_name("URL_OR_MONIKER")
                .takes_value(true)
                .global(true)
                .validator(is_url_or_moniker)
                .help(
                    "URL for put's JSON RPC or moniker (or their first letter): \
                       [mainnet-beta, testnet, devnet, localhost] \
                    Default from the configuration file."
                ),
        )
        .arg(fee_payer_arg().global(true))
        .arg(
            Arg::with_name("use_unchecked_instruction")
                .long("use-unchecked-instruction")
                .takes_value(false)
                .global(true)
                .hidden(true)
                .help("Use unchecked instruction if appropriate. Supports transfer, burn, mint, and approve."),
        )
        .subcommand(
            SubCommand::with_name("create-top")
                .about("Create top domain.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("rule")
                        .long("rule")
                        .takes_value(true)
                        .required(true)
                        .help("The fee rule"),
                )
                .arg(
                    Arg::with_name("space")
                        .long("space")
                        .value_name("space")
                        .takes_value(true)
                        .required(true)
                        .help(
                            "The domain solve account value data max space"),
                )
                .arg(
                    Arg::with_name("proposal")
                        .long("proposal")
                        .takes_value(true)
                        .required(false)
                        .validator(is_valid_pubkey)
                        .help("The created proposal account. \
                         The existence of this parameter means that the verification operation will be performed"),
                )
        )
        .subcommand(
            SubCommand::with_name("create-domain")
                .about("Create first level domain.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("payer")
                        .long("payer")
                        .value_name("payer")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the fee payer keypair."),
                )
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("owner")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the domain owner keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("create-rare-domain")
                .about("Create rare first level domain.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("payer")
                        .long("payer")
                        .value_name("payer")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the fee payer keypair."),
                )
                .arg(
                    Arg::with_name("proposal")
                        .long("proposal")
                        .takes_value(true)
                        .required(false)
                        .validator(is_valid_pubkey)
                        .help("The created proposal account. \
                         The existence of this parameter means that the verification operation will be performed"),
                )
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("owner")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("Specify the domain owner keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("create-resolve-domain")
                .about("Create resolve domain.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("value")
                        .long("value")
                        .value_name("value")
                        .takes_value(true)
                        .required(true)
                        .help("The resolve domain mapped value, must encoded by base58."),
                )
                .arg(
                    Arg::with_name("payer")
                        .long("payer")
                        .value_name("payer")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the fee payer keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("update-resolve-value")
                .about("Update resolve domain value.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("value")
                        .long("value")
                        .value_name("value")
                        .takes_value(true)
                        .required(true)
                        .help("The resolve domain mapped new value, must encoded by base58."),
                )
                .arg(
                    Arg::with_name("payer")
                        .long("payer")
                        .value_name("payer")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the fee payer keypair."),
                )
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("owner")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the the domain owner keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("transfer")
                .about("Transfer a domain to another owner.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain would be transfer"),
                )
                .arg(
                    Arg::with_name("receipt")
                        .long("receipt")
                        .value_name("receipt")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The domain new owner."),
                )
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("owner")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the the domain owner keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("unbind-value")
                .about("Unbind a resolve domain value.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("owner")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the the domain owner keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("close-resolve")
                .about("Close a resolve domain account")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("owner")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the the domain owner keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("renewal")
                .about("Renewal a first level domain.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The domain name"),
                )
                .arg(
                    Arg::with_name("payer")
                        .long("payer")
                        .value_name("payer")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("Specify the domain payer keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("set-receipt")
                .about("Set receipt for top domain account.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The top domain name"),
                )
                .arg(
                    Arg::with_name("receipt")
                        .long("receipt")
                        .value_name("receipt")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_signer)
                        .help("Specify the top domain receipt payer keypair."),
                ),
        )
        .subcommand(
            SubCommand::with_name("account")
                .about("Show the info of the account.")
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .value_name("name")
                        .takes_value(true)
                        .required(true)
                        .help("The account domain name"),
                )
                .arg(
                    Arg::with_name("type")
                        .long("type")
                        .value_name("type")
                        .takes_value(true)
                        .required(true)
                        .help("The account type. [possible values: top, domain, resolve]"),
                ),
        )
        .subcommand(
            SubCommand::with_name("address-info")
                .about("Show the info of the address account.")
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The address account"),
                )
        )
        .subcommand(
            SubCommand::with_name("domains")
                .about("List all domains by owner.")
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        .value_name("address")
                        .validator(is_valid_pubkey)
                        .help("The owner account address"),
                )
                .arg(owner_address_arg())
        )
        .get_matches();

    let mut wallet_manager = None;
    let mut bulk_signers: Vec<Box<dyn Signer>> = Vec::new();
    let mut multisigner_ids = Vec::new();

    let (sub_command, sub_matches) = app_matches.subcommand();
    let matches = sub_matches.unwrap();

    let config = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            put_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            put_cli_config::Config::default()
        };
        let json_rpc_url = normalize_to_url_if_moniker(
            matches
                .value_of("json_rpc_url")
                .unwrap_or(&cli_config.json_rpc_url),
        );
        let _websocket_url = put_cli_config::Config::compute_websocket_url(&json_rpc_url);

        let (signer, fee_payer) = signer_from_path(
            matches,
            matches
                .value_of("fee_payer")
                .unwrap_or(&cli_config.keypair_path),
            "fee_payer",
            &mut wallet_manager,
        )
        .map(|s| {
            let p = s.pubkey();
            (s, p)
        })
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        bulk_signers.push(signer);

        let verbose = matches.is_present("verbose");
        let output_format = matches
            .value_of("output_format")
            .map(|value| match value {
                "json" => OutputFormat::Json,
                "json-compact" => OutputFormat::JsonCompact,
                _ => unreachable!(),
            })
            .unwrap_or(if verbose {
                OutputFormat::DisplayVerbose
            } else {
                OutputFormat::Display
            });

        let nonce_account = pubkey_of_signer(matches, NONCE_ARG.name, &mut wallet_manager)
            .unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                exit(1);
            });
        let nonce_authority = if nonce_account.is_some() {
            let (signer, nonce_authority) = signer_from_path(
                matches,
                matches
                    .value_of(NONCE_AUTHORITY_ARG.name)
                    .unwrap_or(&cli_config.keypair_path),
                NONCE_AUTHORITY_ARG.name,
                &mut wallet_manager,
            )
            .map(|s| {
                let p = s.pubkey();
                (s, p)
            })
            .unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                exit(1);
            });
            bulk_signers.push(signer);

            Some(nonce_authority)
        } else {
            None
        };

        let blockhash_query = BlockhashQuery::new_from_matches(matches);
        let sign_only = matches.is_present(SIGN_ONLY_ARG.name);
        let dump_transaction_message = matches.is_present(DUMP_TRANSACTION_MESSAGE.name);
        let program_id = pubkey_of(matches, "program_id").unwrap();

        let multisig_signers = signers_of(matches, MULTISIG_SIGNER_ARG.name, &mut wallet_manager)
            .unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                exit(1);
            });
        if let Some(mut multisig_signers) = multisig_signers {
            multisig_signers.sort_by(|(_, lp), (_, rp)| lp.cmp(rp));
            let (signers, pubkeys): (Vec<_>, Vec<_>) = multisig_signers.into_iter().unzip();
            bulk_signers.extend(signers);
            multisigner_ids = pubkeys;
        }
        let multisigner_pubkeys = multisigner_ids.iter().collect::<Vec<_>>();

        Config {
            rpc_client: Arc::new(RpcClient::new_with_commitment(
                json_rpc_url,
                CommitmentConfig::confirmed(),
            )),
            _websocket_url,
            output_format,
            fee_payer,
            default_keypair_path: cli_config.keypair_path,
            nonce_account,
            nonce_authority,
            blockhash_query,
            sign_only,
            dump_transaction_message,
            multisigner_pubkeys,
            program_id,
        }
    };

    put_logger::setup_with_default("put=info");

    let result = match (sub_command, sub_matches) {
        ("create-top", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);
            let rule_str = value_t_or_exit!(arg_matches, "rule", String);
            let max_space = value_t_or_exit!(arg_matches, "space", u16);
            let proposal_str = value_t!(arg_matches, "proposal", String);

            let mut verify_proposal_account = None;
            if proposal_str.is_ok() {
                let proposal_puk = Pubkey::from_str(&proposal_str.unwrap()).unwrap();
                verify_proposal_account = Some(proposal_puk);
            }

            let rule = string_to_rule_array(rule_str)?;

            let (sender_signer, payer) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(sender_signer);


            command_create_top_domain(
                &config,
                domain_name,
                rule,
                max_space,
                payer,
                verify_proposal_account,
                bulk_signers
            )
        }

        ("create-domain", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);

            let (sender_signer, payer) = config.signer_or_default(arg_matches, "payer", &mut wallet_manager);
            let (owner_signer, owner) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(sender_signer);
            bulk_signers.push(owner_signer);


            command_create_domain(
                &config,
                domain_name,
                payer,
                owner,
                bulk_signers,
            )
        }

        ("create-rare-domain", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);
            let owner_str = value_t_or_exit!(arg_matches, "owner", String);
            let proposal_str = value_t!(arg_matches, "proposal", String);

            let mut verify_proposal_account = None;
            if proposal_str.is_ok() {
                let proposal_puk = Pubkey::from_str(&proposal_str.unwrap()).unwrap();
                verify_proposal_account = Some(proposal_puk);
            }

            let owner = Pubkey::from_str(owner_str.as_str()).unwrap();
            let (sender_signer, payer) = config.signer_or_default(arg_matches, "payer", &mut wallet_manager);

            bulk_signers.push(sender_signer);


            command_create_rare_domain(
                &config,
                domain_name,
                payer,
                owner,
                verify_proposal_account,
                bulk_signers,
            )
        }

        ("create-resolve-domain", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);
            let value = value_t_or_exit!(arg_matches, "value", String);
            let decode_value = bs58::decode(value).into_vec().unwrap();

            let (sender_signer, payer) = config.signer_or_default(arg_matches, "payer", &mut wallet_manager);
            let (owner_signer, owner) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(sender_signer);
            bulk_signers.push(owner_signer);


            command_create_resolve_domain(
                &config,
                domain_name,
                payer,
                owner,
                decode_value,
                bulk_signers
            )
        }

        ("update-resolve-value", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);
            let new_value = value_t_or_exit!(arg_matches, "value", String);

            let decode_new_value = bs58::decode(new_value).into_vec().unwrap();

            let (sender_signer, payer) = config.signer_or_default(arg_matches, "payer", &mut wallet_manager);
            let (owner_signer, owner) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(sender_signer);
            bulk_signers.push(owner_signer);


            command_update_resolve_domain(
                &config,
                domain_name,
                payer,
                owner,
                decode_new_value,
                bulk_signers
            )
        }


        ("transfer", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);
            let receipt = value_t_or_exit!(arg_matches, "receipt", String);

            let receipt_account = Pubkey::from_str(&receipt).unwrap();

            let (owner_signer, owner) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(owner_signer);


            command_transfer(
                &config,
                domain_name,
                owner,
                receipt_account,
                bulk_signers
            )
        }

        ("unbind-value", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);

            let (owner_signer, owner) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(owner_signer);


            command_unbind_value(
                &config,
                domain_name,
                owner,
                bulk_signers
            )
        }

        ("close-resolve", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);

            let (owner_signer, owner) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);

            bulk_signers.push(owner_signer);


            command_close_resolve(
                &config,
                domain_name,
                owner,
                bulk_signers
            )
        }

        ("renewal", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);

            let (owner_signer, owner) = config.signer_or_default(arg_matches, "payer", &mut wallet_manager);

            bulk_signers.push(owner_signer);


            command_renewal(
                &config,
                domain_name,
                owner,
                bulk_signers
            )
        }

        ("set-receipt", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);

            let (owner_signer, payer) = config.signer_or_default(arg_matches, "owner", &mut wallet_manager);
            let (_, receipt_puk) = get_signer(arg_matches, "receipt", &mut wallet_manager).unwrap();

            bulk_signers.push(owner_signer);


            command_set_top_receipt(
                &config,
                domain_name,
                receipt_puk,
                payer,
                bulk_signers
            )
        }

        ("account", Some(arg_matches)) => {
            let domain_name = value_t_or_exit!(arg_matches, "name", String);
            let account_type = value_t_or_exit!(arg_matches, "type", String);

            command_account(
                &config,
                domain_name,
                account_type,
            )
        }

        ("address-info", Some(arg_matches)) => {
            let address_account_str = value_t_or_exit!(arg_matches, "address", String);
            let value_vec = bs58::decode(address_account_str).into_vec().unwrap();

            command_address_info(
                &config,
                value_vec,
            )
        }

        ("domains", Some(arg_matches)) => {
            let address = value_t!(arg_matches, "address", String).ok();
            let mut owner = config.pubkey_or_default(arg_matches, "owner", &mut wallet_manager);
            if address.is_some() {
                owner = Pubkey::from_str(&*address.unwrap()).unwrap();
            }
            command_domains(&config, owner)
        }

        _ => unreachable!(),
    }
    .map_err::<Error, _>(|err| DisplayError::new_as_boxed(err).into())?;
    println!("{}", result);
    Ok(())
}

fn format_output<T>(command_output: T, command_name: &str, config: &Config) -> String
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    config.output_format.formatted_string(&CommandOutput {
        command_name: String::from(command_name),
        command_output,
    })
}
enum TransactionReturnData {
    CliSignature(CliSignature),
    CliSignOnlyData(CliSignOnlyData),
}
fn handle_tx(
    signer_info: &CliSignerInfo,
    config: &Config,
    no_wait: bool,
    minimum_balance_for_rent_exemption: u128,
    instructions: Vec<Instruction>,
) -> Result<TransactionReturnData, Box<dyn std::error::Error>> {
    let fee_payer = Some(&config.fee_payer);

    let message = if let Some(nonce_account) = config.nonce_account.as_ref() {
        Message::new_with_nonce(
            instructions,
            fee_payer,
            nonce_account,
            config.nonce_authority.as_ref().unwrap(),
        )
    } else {
        Message::new(&instructions, fee_payer)
    };
    let (recent_blockhash, fee_calculator) = config
        .blockhash_query
        .get_blockhash_and_fee_calculator(&config.rpc_client, config.rpc_client.commitment())
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });

    if !config.sign_only {
        check_fee_payer_balance(
            config,
            minimum_balance_for_rent_exemption + fee_calculator.calculate_fee(&message),
        )?;
    }

    let signers = signer_info.signers_for_message(&message);
    let mut transaction = Transaction::new_unsigned(message);

    if config.sign_only {
        transaction.try_partial_sign(&signers, recent_blockhash)?;
        Ok(TransactionReturnData::CliSignOnlyData(return_signers_data(
            &transaction,
            &ReturnSignersConfig {
                dump_transaction_message: config.dump_transaction_message,
            },
        )))
    } else {
        transaction.try_sign(&signers, recent_blockhash)?;
        let signature = if no_wait {
            config.rpc_client.send_transaction(&transaction)?
        } else {
            config
                .rpc_client
                .send_and_confirm_transaction_with_spinner(&transaction)?
        };
        Ok(TransactionReturnData::CliSignature(CliSignature {
            signature: signature.to_string(),
        }))
    }
}

/// Fee rule regex, check the input is valid
fn valid_rule(input: &str) -> Option<&str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\[(([1-9]\d*,)*)+([1-9]\d*)\]$").unwrap();
    }
    if RE.is_match(input) {
        Some(input)
    } else {
        None
    }
}

/// Check the input is valid, and covert it to array
fn string_to_rule_array(rule_str: String) -> Result<[u128; 5], Error> {
    let rule_str = valid_rule(rule_str.as_str());
    if rule_str.is_none() {
        return Err(format!("invalid rule format.").into());
    }
    let maybe_invalid_rule =  rule_str.unwrap();
    let split_rule = maybe_invalid_rule
        .strip_prefix("[").unwrap()
        .strip_suffix("]").unwrap()
        .split(",").collect::<Vec<&str>>();
    if split_rule.len() != 5 {
        return Err(format!("invalid rule format.").into());
    }
    let mut ret = [0 as u128; 5];
    for (index, rule_str) in split_rule.iter().enumerate() {
        let fee = u128::from_str(rule_str);
        if fee.is_err() {
            return Err(format!("invalid rule format.").into());
        }
        ret[index] = fee.unwrap()
    }
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use crate::string_to_rule_array;

    #[test]
    fn test_string_to_rule_array() {
        let valid_rule1 = "[1,23,45,17,68]";
        let invalid_rule1 = "[1,23,45,17,68,78]";
        let invalid_rule2 = "[01,23,45,17,68]";
        let invalid_rule3 = "[-1,23,45,17,68]";
        let invalid_rule4 = "[23,45,17,68]";

        assert!(string_to_rule_array(valid_rule1.to_string()).is_ok());
        assert!(string_to_rule_array(invalid_rule1.to_string()).is_err());
        assert!(string_to_rule_array(invalid_rule2.to_string()).is_err());
        assert!(string_to_rule_array(invalid_rule3.to_string()).is_err());
        assert!(string_to_rule_array(invalid_rule4.to_string()).is_err());
    }
}
