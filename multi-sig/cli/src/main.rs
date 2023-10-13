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
use put_sdk::{commitment_config::CommitmentConfig, instruction::Instruction, message::Message, native_token::*, pubkey::Pubkey, signature::{Signer}, system_instruction, system_program, transaction::Transaction};
// use spl_associated_token_account::{
//     get_associated_token_address_with_program_id, instruction::create_associated_token_account,
// };
use chrono::{DateTime, Utc};


use std::{fmt::Display, io, process::exit, str::FromStr, sync::Arc};
use std::fs::File;
use std::io::BufRead;
use std::ops::Add;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use borsh::BorshDeserialize;
use put_clap_utils::offline::{DUMP_TRANSACTION_MESSAGE, SIGN_ONLY_ARG};
use put_sdk::signature::Keypair;

mod config;
use config::Config;

mod output;
use output::*;
use ppl_sig::instruction::{create_add_signer, create_close_proposal_ins, create_multi_sig_account, create_proposal_account, create_remove_signer, create_set_new_threshold, create_vote_ins, verify};
use ppl_sig::state::{AccountState, MultiSigAccount, ProposalAccount};
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

pub const MULTISIG_SIGNER_ARG: ArgConstant<'static> = ArgConstant {
    name: "multisig_signer",
    long: "multisig-signer",
    help: "Member signer of a multisig account",
};


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


pub(crate) type Error = Box<dyn std::error::Error>;

pub(crate) type CommandResult = Result<String, Error>;

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

fn new_throwaway_signer() -> (Box<dyn Signer>, Pubkey) {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    (Box::new(keypair) as Box<dyn Signer>, pubkey)
}

#[allow(clippy::too_many_arguments)]
fn command_create_multi_sig_account(
    config: &Config,
    multi_sig_account: Pubkey,
    sig_accounts: Vec<Pubkey>,
    threshold: u8,
    payer: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("creating multi sig account {}", multi_sig_account));

    let instructions = vec![
        system_instruction::create_account(
            &payer,
            &multi_sig_account,
            0,
            0,
            &system_program::id()
        ),
        create_multi_sig_account(
            &config.program_id,
            sig_accounts,
            threshold,
            &multi_sig_account,
            &payer
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
            format_output(cli_sign_only_data, "create-proposal", config)
        }
    })
}



#[allow(clippy::too_many_arguments)]
fn command_create_proposal_account(
    config: &Config,
    multi_sig_account_puk: Pubkey,
    summary: String,
    payer: Pubkey,
    validity_period: u32,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_puk).unwrap();

    let multi_sig_account_obj : MultiSigAccount =
        MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).expect(&format!("Multi sig {} account not exist", multi_sig_account_puk));

    let (proposal_account_puk, _) =
        find_proposal_account(&multi_sig_account_puk, multi_sig_account_obj.nonce + 1);
    // todo delete
    // let (proposal_signer, proposal_account_puk) = new_throwaway_signer();
    // bulk_signers.push(proposal_signer);

    println_display(config, format!("creating proposal account {}", proposal_account_puk));

    let instructions = vec![
        create_proposal_account(
            &config.program_id,
            summary,
            validity_period,
            &multi_sig_account_puk,
            &payer,
            &proposal_account_puk,

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
            format_output(cli_sign_only_data, "create-sig-account", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_vote(
    config: &Config,
    signer_puk: Pubkey,
    proposal_account_puk: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("voting for a proposal {}", proposal_account_puk));
    let proposal_account = config.rpc_client.get_account(&proposal_account_puk).unwrap();

    let proposal_account_obj : ProposalAccount = ProposalAccount::deserialize(&mut proposal_account.data.as_slice()).unwrap();
    let multi_sig_account = proposal_account_obj.parent;



    let instructions = vec![
        create_vote_ins(
            &config.program_id,
            &multi_sig_account,
            &proposal_account_puk,
            &signer_puk
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
            format_output(cli_sign_only_data, "vote", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_verify(
    config: &Config,
    initiator: Pubkey,
    proposal_account_puk: Pubkey,
    summary: String,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("verifying proposal {} passed ", proposal_account_puk));
    let proposal_account = config.rpc_client.get_account(&proposal_account_puk).unwrap();

    let proposal_account_obj : ProposalAccount = ProposalAccount::deserialize(&mut proposal_account.data.as_slice()).unwrap();
    let multi_sig_account = proposal_account_obj.parent;



    let instructions = vec![
        verify(
            &config.program_id,
            &multi_sig_account,
            &initiator,
            &proposal_account_puk,
            Some(summary)
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
            format_output(cli_sign_only_data, "verify", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_add_signer(
    config: &Config,
    multi_sig_account_puk: Pubkey,
    initiator_account: Pubkey,
    new_signer: Pubkey,
    verify_proposal_account: Option<Pubkey>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("add new signer in multi sig account {} ", multi_sig_account_puk));
    let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_puk).unwrap();


    let multi_sig_account_obj : MultiSigAccount =
        MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).expect(&format!("Multi sig {} account not exist", multi_sig_account_puk));
    let proposal_account_puk : Pubkey;
    if verify_proposal_account.is_some() {
        proposal_account_puk = verify_proposal_account.unwrap();
    } else {
        let (found_proposal_account_puk, _) =
            find_proposal_account(&multi_sig_account_puk, multi_sig_account_obj.nonce + 1);
        proposal_account_puk = found_proposal_account_puk;
    }


    let instructions = vec![
        create_add_signer(
            &config.program_id,
            &multi_sig_account_puk,
            &initiator_account,
            &proposal_account_puk,
            new_signer,
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
            format_output(cli_sign_only_data, "add-signer", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_remove_signer(
    config: &Config,
    multi_sig_account_puk: Pubkey,
    initiator_account: Pubkey,
    drop_signer: Pubkey,
    verify_proposal_account: Option<Pubkey>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("removing signer {} from multi sig account {} ", drop_signer, multi_sig_account_puk));
    let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_puk).unwrap();

    let multi_sig_account_obj : MultiSigAccount =
        MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).expect(&format!("Multi sig {} account not exist", multi_sig_account_puk));


    let proposal_account_puk : Pubkey;
    if verify_proposal_account.is_some() {
        proposal_account_puk = verify_proposal_account.unwrap();
    } else {
        let (found_proposal_account_puk, _) =
            find_proposal_account(&multi_sig_account_puk, multi_sig_account_obj.nonce + 1);
        proposal_account_puk = found_proposal_account_puk;
    }

    let instructions = vec![
        create_remove_signer(
            &config.program_id,
            &multi_sig_account_puk,
            &initiator_account,
            &proposal_account_puk,
            drop_signer,
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
            format_output(cli_sign_only_data, "add-signer", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_proposal_info(
    config: &Config,
    proposal_account_puk: Pubkey,
) -> CommandResult {
    let proposal_account = config.rpc_client.get_account(&proposal_account_puk).unwrap();

    let proposal_account_obj : ProposalAccount =
        ProposalAccount::deserialize(&mut proposal_account.data.as_slice()).expect(&format!("Proposal {} account not exist", proposal_account_puk));

    let multi_sig_account = config.rpc_client.get_account(&proposal_account_obj.parent).unwrap();

    let multi_sig_account_obj : MultiSigAccount =
        MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).expect(&format!("Multi sig {} account not exist or closed", proposal_account_puk));
    let pass_tickets = proposal_account_obj.count_tickets();
    let total_tickets = multi_sig_account_obj.get_valid_tickets_count();
    let mut proposal_status = "Voting";
    if pass_tickets * 100 > total_tickets * multi_sig_account_obj.threshold as usize{
        proposal_status = "Passed";
    }
    let duration = Duration::from_secs(proposal_account_obj.tx_expired_time as u64);
    let expired_time = DateTime::<Utc>::from(UNIX_EPOCH.add(duration));
    // Formats the combined date and time with the specified format string.
    let ui_expired_time = expired_time.format("%Y-%m-%d %H:%M:%S").to_string();
    println!();
    println!("Initiator: {} ", proposal_account_obj.initiator);
    println!("Multi sig account: {} ", proposal_account_obj.parent);
    println!("Proposal summary: \"{}\" ", proposal_account_obj.summary);
    println!("Chain expire time : {} ", ui_expired_time);
    println!("Proposal status: {}", proposal_account_obj.account_state.to_string());
    println!("Proposal vote status: {}", proposal_status);
    println!("----------------Voted account----------------");
    println!();
    for x in proposal_account_obj.tickets {
        if x.1 {
            println!("{}", x.0)
        } else {
            break
        }
    }

    Ok("".to_string())
}

#[allow(clippy::too_many_arguments)]
fn command_sig_info(
    config: &Config,
    multi_sig_puk: Pubkey,
) -> CommandResult {
    let multi_sig_account = config.rpc_client.get_account(&multi_sig_puk).unwrap();

    let multi_sig_data_obj : MultiSigAccount = MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).unwrap();
    println!();
    println!("Pass threshold: {}% ", multi_sig_data_obj.threshold);
    println!("Latest nonce: {}", multi_sig_data_obj.nonce);
    println!("----------------all signers----------------");
    println!();
    for x in multi_sig_data_obj.accounts {
        if x.1 {
            println!("{}", x.0)
        }
    }

    Ok("".to_string())
}

#[allow(clippy::too_many_arguments)]
fn command_set_threshold(
    config: &Config,
    multi_sig_account_puk: Pubkey,
    initiator_account: Pubkey,
    new_threshold: u8,
    verify_proposal_account: Option<Pubkey>,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("changing multi sig account {} threshold to {} ", multi_sig_account_puk, new_threshold));
    let multi_sig_account = config.rpc_client.get_account(&multi_sig_account_puk).unwrap();

    let multi_sig_account_obj : MultiSigAccount =
        MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).expect(&format!("Multi sig {} account not exist", multi_sig_account_puk));


    let proposal_account_puk : Pubkey;
    if verify_proposal_account.is_some() {
        proposal_account_puk = verify_proposal_account.unwrap();
    } else {
        let (found_proposal_account_puk, _) =
            find_proposal_account(&multi_sig_account_puk, multi_sig_account_obj.nonce + 1);
        proposal_account_puk = found_proposal_account_puk;
    }

    let instructions = vec![
        create_set_new_threshold(
            &config.program_id,
            &multi_sig_account_puk,
            &initiator_account,
            &proposal_account_puk,
            new_threshold,
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
            format_output(cli_sign_only_data, "set-threshold", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_close_proposal(
    config: &Config,
    proposal_account_puk: Pubkey,
    initiator_account: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("{} is closing proposal account {}", initiator_account, proposal_account_puk));

    let proposal_account = config.rpc_client.get_account(&proposal_account_puk).unwrap();

    let proposal_account_obj : ProposalAccount =
        ProposalAccount::deserialize(&mut proposal_account.data.as_slice()).expect(&format!("Proposal {} account not exist", proposal_account_puk));

    let instructions = vec![
        create_close_proposal_ins(
            &config.program_id,
            &proposal_account_puk,
            &initiator_account,
            &proposal_account_obj.parent
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
            format_output(cli_sign_only_data, "close-proposal", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_list_proposals(
    config: &Config,
    multi_sig_puk: Pubkey,
) -> CommandResult {
    let multi_sig_account = config.rpc_client.get_account(&multi_sig_puk).unwrap();

    let multi_sig_data_obj : MultiSigAccount = MultiSigAccount::deserialize(&mut multi_sig_account.data.as_slice()).unwrap();
    let all_proposals = (1..=multi_sig_data_obj.nonce)
        .into_iter()
        .map(|x| -> Pubkey {
        let (proposal_puk, _) = find_proposal_account(&multi_sig_puk, x);
        proposal_puk
    }).collect::<Vec<Pubkey>>();
    println!();
    println!("-------------------- all proposals --------------------");
    println!();
    for x in all_proposals {
        let proposal_account = config.rpc_client.get_account(&x);
        if proposal_account.is_err() {
            println!("{}({})", x, "Closed");
            continue;
        }
        let proposal_account_obj : ProposalAccount =
            ProposalAccount::deserialize(&mut proposal_account.unwrap().data.as_slice()).expect(&format!("Find a invalid proposal account [{}]", x));

        if proposal_account_obj.account_state == AccountState::Verified {
            println!("{}({})", x, "Verified");
            continue;
        }
        if SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() > proposal_account_obj.tx_expired_time as u64 {
            println!("{}({})", x, "Expired");
            continue;
        }

        println!("{}({})", x, "Signable")
    }

    Ok("".to_string())
}




fn main() -> Result<(), Error> {
    // let default_decimals = &format!("{}", native_mint::DECIMALS);
    let default_program_id = ppl_sig::id().to_string();
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
                .help("PPL multi sig program id"),
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
            SubCommand::with_name("create-sig-account")
                .about("Create multi sig account.")
                .arg(
                    Arg::with_name("threshold")
                        .long("threshold")
                        .value_name("threshold")
                        .takes_value(true)
                        .required(true)
                        .help("The threshold of passing a tx proposal. \
                                [possible value 1-100, eg:input 51, means need above 51% signer agress]"),
                )
                .arg(
                    Arg::with_name("accounts")
                        .long("accounts")
                        .takes_value(true)
                        .required(true)
                        .help("The path of signer public keys， separated by line."),
                )
                .arg(
                    Arg::with_name("multi-keypair")
                        .long("mk")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The one of multi sig account keypair path."),
                )
        )
        .subcommand(
            SubCommand::with_name("create-proposal")
                .about("Create proposal account.")
                .arg(
                    Arg::with_name("multi-sig-account")
                        .long("multi-sig-account")
                        .short("msa")
                        .takes_value(true)
                        .validator(is_valid_pubkey)
                        .required(true)
                        .help("The multi-sig-account."),
                )
                .arg(
                    Arg::with_name("summary")
                        .long("summary")
                        .takes_value(true)
                        .required(true)
                        .help("The summary of the transaction."),
                )
                .arg(
                    Arg::with_name("signer")
                        .long("sig")
                        .takes_value(true)
                        .required(false)
                        .help("The proposal initiator key-pair"),
                )
                .arg(
                    Arg::with_name("validity-period")
                        .short("v")
                        .takes_value(true)
                        .required(false)
                        .help("The proposal continuous validity period, in seconds. \
                            After this time, the proposal will be expired. \
                            Common base time: 60(1 Minute), 3600(1 hour), 86400(1 day).
                            "),
                )
        )
        .subcommand(
            SubCommand::with_name("vote")
                .about("Vote for a proposal to execute.")
                .arg(
                    Arg::with_name("signer")
                        .long("signer")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The one of multi signers keypair path."),
                )
                .arg(
                    Arg::with_name("proposal")
                        .long("proposal")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The proposal account"),
                )
        )
        .subcommand(
            SubCommand::with_name("verify")
                .about("verify a proposal passed.")
                .arg(
                    Arg::with_name("initiator")
                        .long("initiator")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The initiator keypair path."),
                )
                .arg(
                    Arg::with_name("proposal")
                        .long("proposal")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The proposal account"),
                )
                .arg(
                    Arg::with_name("summary")
                        .long("summary")
                        .takes_value(true)
                        .required(true)
                        .help("The summary plain of the proposal."),
                )
        )
        .subcommand(
            SubCommand::with_name("add-signer")
                .about("add a new signer to multi sig account.")
                .arg(
                    Arg::with_name("admin")
                        .long("admin")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The multi sig account's admin keypair path."),
                )
                .arg(
                    Arg::with_name("multi-sig-account")
                        .long("msa")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The multi sig account"),
                )
                .arg(
                    Arg::with_name("signer")
                        .short("signer")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The new signer"),
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
            SubCommand::with_name("remove-signer")
                .about("remove a signer from multi sig account.")
                .arg(
                    Arg::with_name("admin")
                        .long("admin")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The multi sig account's admin keypair path."),
                )
                .arg(
                    Arg::with_name("multi-sig-account")
                        .long("msa")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The multi sig account"),
                )
                .arg(
                    Arg::with_name("signer")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The signer will removed."),
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
            SubCommand::with_name("proposal-info")
                .about("Show the proposal details.")
                .arg(
                    Arg::with_name("proposal-address")
                        .index(1)
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The proposal account address."),
                )
        )
        .subcommand(
            SubCommand::with_name("multi-info")
                .about("Show the multi sig account details.")
                .arg(
                    Arg::with_name("multi-sig-account")
                        .index(1)
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The multi sig account address."),
                )
        )
        .subcommand(
            SubCommand::with_name("set-threshold")
                .about("Set a new threshold for a multi sig account.")
                .arg(
                    Arg::with_name("admin")
                        .long("admin")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The multi sig account's admin keypair path."),
                )
                .arg(
                    Arg::with_name("multi-sig-account")
                        .long("msa")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The multi sig account"),
                )
                .arg(
                    Arg::with_name("new-threshold")
                        .long("nt")
                        .takes_value(true)
                        .required(true)
                        .help("The new threshold for updating"),
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
            SubCommand::with_name("close-proposal")
                .about("Close a proposal account")
                .arg(
                    Arg::with_name("admin")
                        .long("adm")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The proposal initiator account"),
                )
                .arg(
                    Arg::with_name("proposal-account")
                        .long("pa")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The proposal account"),
                )
        )
        .subcommand(
            SubCommand::with_name("list-proposal")
                .about("List all proposals of multi sig account")
                .arg(
                    Arg::with_name("multi-sig-account")
                        .long("msa")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("The multi sig account"),
                )
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
        ("create-sig-account", Some(arg_matches)) => {
            let threshold = value_t_or_exit!(arg_matches, "threshold", u8);
            let accounts_path = value_t_or_exit!(arg_matches, "accounts", String);
            let multi_keypair_path = value_t!(arg_matches, "multi-keypair", String);

            // Read signer public key list
            let mut signer_list: Vec<Pubkey> = Vec::new();
            let file = File::open(accounts_path).unwrap();
            let lines = io::BufReader::new(file).lines();

            for line in lines {
                if let Ok(pubkey_str) = line {
                    signer_list.push(Pubkey::from_str(&pubkey_str).unwrap());
                }
            }

            let (mut multi_sig_account_signer,mut multi_sig_puk) = new_throwaway_signer();

            if multi_keypair_path.is_ok() {
                let (multi_sig_signer, address) = config.signer_or_default(arg_matches, "multi-keypair", &mut wallet_manager);
                multi_sig_account_signer = multi_sig_signer;
                multi_sig_puk = address;
            }

            bulk_signers.push(multi_sig_account_signer);

            command_create_multi_sig_account(
                &config,
                multi_sig_puk,
                signer_list,
                threshold,
                config.fee_payer.clone(),
                bulk_signers
            )
        }

        ("create-proposal", Some(arg_matches)) => {
            let multi_sig_account = value_t_or_exit!(arg_matches, "multi-sig-account", String);
            let summary = value_t_or_exit!(arg_matches, "summary", String);
            let validity_period = value_t_or_exit!(arg_matches, "validity-period", u32);

            let multi_sig_account_puk  = Pubkey::from_str(&multi_sig_account).unwrap();

            let (signer, payer) = config.signer_or_default(arg_matches, "signer", &mut wallet_manager);

            bulk_signers.push(signer);


            command_create_proposal_account(
                &config,
                multi_sig_account_puk,
                summary,
                payer,
                validity_period,
                bulk_signers
            )
        }

        ("vote", Some(arg_matches)) => {
            let proposal_account = value_t_or_exit!(arg_matches, "proposal", String);

            let proposal_account_puk  = Pubkey::from_str(&proposal_account).unwrap();

            let (proposal_signer, signer_puk) = config.signer_or_default(arg_matches, "signer", &mut wallet_manager);

            bulk_signers.push(proposal_signer);


            command_vote(
                &config,
                signer_puk,
                proposal_account_puk,
                bulk_signers,
            )
        }

        ("verify", Some(arg_matches)) => {
            let proposal_account = value_t_or_exit!(arg_matches, "proposal", String);

            let proposal_account_puk  = Pubkey::from_str(&proposal_account).unwrap();
            let summary = value_t_or_exit!(arg_matches, "summary", String);

            let (initiator_signer, initiator_puk) = config.signer_or_default(arg_matches, "initiator", &mut wallet_manager);

            bulk_signers.push(initiator_signer);


            command_verify(
                &config,
                initiator_puk,
                proposal_account_puk,
                summary,
                bulk_signers,
            )
        }

        ("add-signer", Some(arg_matches)) => {
            let multi_sig_account = value_t_or_exit!(arg_matches, "multi-sig-account", String);
            let new_signer = value_t_or_exit!(arg_matches, "signer", String);
            let proposal_str = value_t!(arg_matches, "proposal", String);

            let mut verify_proposal_account = None;
            if proposal_str.is_ok() {
                let proposal_puk = Pubkey::from_str(&proposal_str.unwrap()).unwrap();
                verify_proposal_account = Some(proposal_puk);
            }

            let multi_sig_account_puk  = Pubkey::from_str(&multi_sig_account).unwrap();
            let new_signer  = Pubkey::from_str(&new_signer).unwrap();

            let (initiator_signer, initiator_account) = config.signer_or_default(arg_matches, "admin", &mut wallet_manager);

            bulk_signers.push(initiator_signer);



            command_add_signer(
                &config,
                multi_sig_account_puk,
                initiator_account,
                new_signer,
                verify_proposal_account,
                bulk_signers
            )
        }

        ("remove-signer", Some(arg_matches)) => {
            let multi_sig_account = value_t_or_exit!(arg_matches, "multi-sig-account", String);
            let drop_signer = value_t_or_exit!(arg_matches, "signer", String);
            let proposal_str = value_t!(arg_matches, "proposal", String);

            let mut verify_proposal_account = None;
            if proposal_str.is_ok() {
                let proposal_puk = Pubkey::from_str(&proposal_str.unwrap()).unwrap();
                verify_proposal_account = Some(proposal_puk);
            }

            let multi_sig_account_puk  = Pubkey::from_str(&multi_sig_account).unwrap();
            let drop_signer  = Pubkey::from_str(&drop_signer).unwrap();

            let (initiator_signer, initiator_account) = config.signer_or_default(arg_matches, "admin", &mut wallet_manager);

            bulk_signers.push(initiator_signer);


            command_remove_signer(
                &config,
                multi_sig_account_puk,
                initiator_account,
                drop_signer,
                verify_proposal_account,
                bulk_signers
            )
        }

        ("proposal-info", Some(arg_matches)) => {
            let proposal_account = value_t_or_exit!(arg_matches, "proposal-address", String);

            let proposal_account_puk  = Pubkey::from_str(&proposal_account).unwrap();
            command_proposal_info(
                &config,
                proposal_account_puk,
            )
        }

        ("multi-info", Some(arg_matches)) => {
            let multi_sig_account = value_t_or_exit!(arg_matches, "multi-sig-account", String);

            let multi_sig_account_puk  = Pubkey::from_str(&multi_sig_account).unwrap();
            command_sig_info(
                &config,
                multi_sig_account_puk,
            )
        }

        ("set-threshold", Some(arg_matches)) => {
            let multi_sig_account = value_t_or_exit!(arg_matches, "multi-sig-account", String);
            let new_threshold = value_t_or_exit!(arg_matches, "new-threshold", u8);
            let proposal_str = value_t!(arg_matches, "proposal", String);

            let mut verify_proposal_account = None;
            if proposal_str.is_ok() {
                let proposal_puk = Pubkey::from_str(&proposal_str.unwrap()).unwrap();
                verify_proposal_account = Some(proposal_puk);
            }

            let multi_sig_account_puk  = Pubkey::from_str(&multi_sig_account).unwrap();

            let (initiator_signer, initiator_account) = config.signer_or_default(arg_matches, "admin", &mut wallet_manager);
            bulk_signers.push(initiator_signer);

            command_set_threshold(
                &config,
                multi_sig_account_puk,
                initiator_account,
                new_threshold,
                verify_proposal_account,
                bulk_signers
            )
        }

        ("close-proposal", Some(arg_matches)) => {
            let proposal_account = value_t_or_exit!(arg_matches, "proposal-account", String);

            let proposal_account_puk  = Pubkey::from_str(&proposal_account).unwrap();

            let (initiator_signer, initiator_account) = config.signer_or_default(arg_matches, "admin", &mut wallet_manager);
            bulk_signers.push(initiator_signer);

            command_close_proposal(
                &config,
                proposal_account_puk,
                initiator_account,
                bulk_signers
            )
        }

        ("list-proposal", Some(arg_matches)) => {
            let multi_sig_account = value_t_or_exit!(arg_matches, "multi-sig-account", String);

            let multi_sig_account_puk  = Pubkey::from_str(&multi_sig_account).unwrap();
            command_list_proposals(
                &config,
                multi_sig_account_puk,
            )
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
