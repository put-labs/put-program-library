#![allow(deprecated)] // TODO: Remove when SPL upgrades to put 1.8
use clap::{crate_description, crate_name, crate_version, value_t, value_t_or_exit, App, AppSettings, Arg, ArgMatches, SubCommand};
use serde::Serialize;

use put_clap_utils::{
    fee_payer::fee_payer_arg,
    input_parsers::{pubkey_of, pubkey_of_signer},
    input_validators::{
        is_parsable, is_url_or_moniker, is_valid_pubkey,
        is_valid_signer, normalize_to_url_if_moniker,
    },
    keypair::{signer_from_path, CliSignerInfo},
    memo::memo_arg,
    nonce::*,
    offline::{self, *},
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
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::Transaction,
};
// use spl_associated_token_account::{
//     get_associated_token_address_with_program_id, instruction::create_associated_token_account,
// };


use std::{fmt::Display, io, process::exit, str::FromStr, sync::Arc};
use std::convert::TryFrom;
use std::fs::File;
use std::io::BufRead;
use std::ops::Deref;
use put_client::rpc_request::TokenAccountsFilter;
use put_sdk::account::ReadableAccount;
use put_sdk::program_pack::Pack;

mod config;
use config::Config;

mod output;
use output::*;

mod sort;
use sort::sort_and_parse_token_accounts;

use ppl_nft::instruction::{update_instruction, create_mint_to_inst, create_transfer_inst, initialize_mint, create_freeze_instruction, create_burn_instruction, AuthorityType, create_authorize_instruction, create_thaw_instruction, UpdateType};
// use ppl_nft::put_program::program_pack::Pack;
use ppl_nft::state::{MetaAccount, NftMint};

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

fn is_mint_supply(string: String) -> Result<(), String> {
    is_parsable::<u64>(string)
}

// pub fn mint_decimals_arg<'a, 'b>() -> Arg<'a, 'b> {
//     Arg::with_name(MINT_DECIMALS_ARG.name)
//         .long(MINT_DECIMALS_ARG.long)
//         .takes_value(true)
//         .value_name("MINT_DECIMALS")
//         .validator(is_mint_decimals)
//         .requires(SIGN_ONLY_ARG.name)
//         .requires(BLOCKHASH_ARG.name)
//         .help(MINT_DECIMALS_ARG.help)
// }

pub trait MintArgs {
    fn mint_args(self) -> Self;
}

// impl MintArgs for App<'_, '_> {
//     fn mint_args(self) -> Self {
//         self.arg(mint_address_arg().requires(MINT_DECIMALS_ARG.name))
//             .arg(mint_decimals_arg().requires(MINT_ADDRESS_ARG.name))
//     }
// }

pub fn delegate_address_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name(DELEGATE_ADDRESS_ARG.name)
        .long(DELEGATE_ADDRESS_ARG.long)
        .takes_value(true)
        .value_name("DELEGATE_ADDRESS")
        .validator(is_valid_pubkey)
        .requires(SIGN_ONLY_ARG.name)
        .requires(BLOCKHASH_ARG.name)
        .help(DELEGATE_ADDRESS_ARG.help)
}

// pub fn multisig_signer_arg<'a, 'b>() -> Arg<'a, 'b> {
//     Arg::with_name(MULTISIG_SIGNER_ARG.name)
//         .long(MULTISIG_SIGNER_ARG.long)
//         .validator(is_valid_signer)
//         .value_name("MULTISIG_SIGNER")
//         .takes_value(true)
//         .multiple(true)
//         .min_values(0u64)
//         .max_values(MAX_SIGNERS as u64)
//         .help(MULTISIG_SIGNER_ARG.help)
// }

// fn is_multisig_minimum_signers(string: String) -> Result<(), String> {
//     let v = u8::from_str(&string).map_err(|e| e.to_string())? as usize;
//     if v < MIN_SIGNERS {
//         Err(format!("must be at least {}", MIN_SIGNERS))
//     } else if v > MAX_SIGNERS {
//         Err(format!("must be at most {}", MAX_SIGNERS))
//     } else {
//         Ok(())
//     }
// }

pub(crate) type Error = Box<dyn std::error::Error>;

pub(crate) type CommandResult = Result<String, Error>;

fn new_throwaway_signer() -> (Box<dyn Signer>, Pubkey) {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    (Box::new(keypair) as Box<dyn Signer>, pubkey)
}

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

#[allow(clippy::too_many_arguments)]
fn command_create_token(
    config: &Config,
    total_supply: u64,
    token: Pubkey,
    authority: Pubkey,
    enable_freeze: bool,
    _: Option<String>,
    name: String,
    symbol: String,
    icon_uri: String,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("Creating token {}", token));

    let minimum_balance_for_rent_exemption = 0;
    let freeze_authority_pubkey = if enable_freeze { Some(authority) } else { None };

    println!("program_id {}", config.program_id.to_string());
    let instructions = vec![
        system_instruction::create_account(
            &config.fee_payer,
            &token.clone(),
            minimum_balance_for_rent_exemption,
            0,
            &system_program::id()
        ),
        initialize_mint(
            config.program_id,
            token,
            config.fee_payer,
            freeze_authority_pubkey,
            total_supply,
            name,
            symbol,
            icon_uri.clone()
        )?,
    ];
    // if let Some(text) = memo {
    //     instructions.push(spl_memo::build_memo(text.as_bytes(), &[&config.fee_payer]));
    // }

    let tx_return = handle_tx(
        &CliSignerInfo {
            signers: bulk_signers,
        },
        config,
        false,
        minimum_balance_for_rent_exemption,
        instructions,
    )?;

    Ok(match tx_return {
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliCreateMint {
                address: token.to_string(),
                total_supply,
                icon_uri,
                transaction_data: cli_signature,
            },
            CREATE_TOKEN,
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, CREATE_TOKEN, config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_mint(
    config: &Config,
    mint: Pubkey,
    token: Pubkey,
    owner: Pubkey,
    token_uri: String,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("minting a nft {}", token));

    let instructions = vec![
        create_mint_to_inst(
            token,
            mint,
            owner,
            config.program_id,
            token_uri.clone(),
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
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliMintTo {
                address: token.to_string(),
                token_uri,
                transaction_data: cli_signature,
            },
            "mint",
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "mint", config)
        }
    })
}


fn command_accounts(config: &Config, mint: Option<Pubkey>, owner: Pubkey) -> CommandResult {
    println!("find nft byt owner, program id {}, owner {}", config.program_id.to_string(), owner.to_string());
    if let Some(mint) = mint {
        validate_mint(config, mint)?;
    }
    let accounts = config.rpc_client.get_nft_accounts_by_owner(
        &owner,
        match mint {
            Some(mint) => TokenAccountsFilter::Mint(mint),
            None => TokenAccountsFilter::ProgramId(config.program_id),
        },
    )?;
    if accounts.is_empty() {
        println!("None");
        return Ok("".to_string());
    }
    let (mint_accounts, unsupported_accounts, max_token_id_len) =
        sort_and_parse_token_accounts(accounts);

    let cli_nft_accounts = CliNftAccounts {
        accounts: mint_accounts
            .into_iter()
            .map(|(_mint, accounts_list)| accounts_list)
            .collect(),
        unsupported_accounts,
        max_token_id_len,
        token_is_some: mint.is_some()
    };
    Ok(config.output_format.formatted_string(&cli_nft_accounts))
}

#[allow(clippy::too_many_arguments)]
fn command_authorize(
    config: &Config,
    address: Pubkey,
    new_authority: Option<Pubkey>,
    auth_type: AuthorityType,
    old_owner: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("authorize a {:?} authority to  {}", auth_type , address));

    let instructions = vec![
        create_authorize_instruction(
            address.clone(),
            new_authority.clone(),
            auth_type.clone(),
            old_owner,
            config.program_id,
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
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliAuthorize {
                authorize_account: address.to_string(),
                new_authority,
                //todo auth_type to_string
                auth_type: "auth_type".to_string(),
                transaction_data: cli_signature,
            },
            "authorize",
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "authorize", config)
        }
    })
}


#[allow(clippy::too_many_arguments)]
fn command_burn(
    config: &Config,
    token: Pubkey,
    freeze_authority: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("burning a nft {}", token));

    let instructions = vec![
        create_burn_instruction(
            token,
            freeze_authority,
            config.program_id,
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
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliBurn {
                burn_nft: token.to_string(),
                transaction_data: cli_signature,
            },
            "burn",
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "burn", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_thaw(
    config: &Config,
    mint: Pubkey,
    token: Pubkey,
    thaw_authority: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("thawing a nft {}", token));

    let instructions = vec![
        create_thaw_instruction(
            token,
            thaw_authority,
            mint,
            config.program_id,
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
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliThaw {
                thaw_nft: token.to_string(),
                transaction_data: cli_signature,
            },
            "thaw",
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "thaw", config)
        }
    })
}


#[allow(clippy::too_many_arguments)]
fn command_freeze(
    config: &Config,
    mint: Pubkey,
    token: Pubkey,
    freeze_authority: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("freezing a nft {}", token));

    let instructions = vec![
        create_freeze_instruction(
            token,
            freeze_authority,
            mint,
            config.program_id,
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
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliFreeze {
                freeze_nft: token.to_string(),
                transaction_data: cli_signature,
            },
            "freeze",
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "freeze", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_transfer(
    config: &Config,
    sender: Pubkey,
    recipient: Pubkey,
    nft: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    println_display(config, format!("transfer the nft {} from {} to {}", nft, sender, recipient));

    println!("program_id {}", config.program_id.to_string());
    let instructions = vec![

        create_transfer_inst(
            sender,
            recipient,
            nft,
            config.program_id,
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
        TransactionReturnData::CliSignature(cli_signature) => format_output(
            CliTransfer {
                from: sender.to_string(),
                to: recipient.to_string(),
                nft: nft.to_string(),
                transaction_data: cli_signature,
            },
            "freeze",
            config,
        ),
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "freeze", config)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn command_update(
    config: &Config,
    update_type: UpdateType,
    address_pubkey: Pubkey,
    bulk_signers: Vec<Box<dyn Signer>>,
) -> CommandResult {
    let instructions = vec![
        update_instruction(
            address_pubkey,
            config.fee_payer,
            update_type.clone(),
            config.program_id,
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
            cli_signature.signature.to_string()
        }
        ,
        TransactionReturnData::CliSignOnlyData(cli_sign_only_data) => {
            format_output(cli_sign_only_data, "update-nft", config)
        }
    })
}



// fn command_accounts(config: &Config, token: Option<Pubkey>, owner: Pubkey) -> CommandResult {
//     if let Some(token) = token {
//         validate_mint(config, token)?;
//     }
//     let accounts = config.rpc_client.get_token_accounts_by_owner(
//         &owner,
//         match token {
//             Some(token) => TokenAccountsFilter::Mint(token),
//             None => TokenAccountsFilter::ProgramId(config.program_id),
//         },
//     )?;
//     if accounts.is_empty() {
//         println!("None");
//         return Ok("".to_string());
//     }
//
//     let (mint_accounts, unsupported_accounts, max_len_balance, includes_aux) =
//         sort_and_parse_token_accounts(&owner, accounts, &config.program_id);
//     let aux_len = if includes_aux { 10 } else { 0 };
//
//     let cli_token_accounts = CliTokenAccounts {
//         accounts: mint_accounts
//             .into_iter()
//             .map(|(_mint, accounts_list)| accounts_list)
//             .collect(),
//         unsupported_accounts,
//         max_len_balance,
//         aux_len,
//         token_is_some: token.is_some(),
//     };
//     Ok(config.output_format.formatted_string(&cli_token_accounts))
// }


// fn command_account_info(config: &Config, address: Pubkey) -> CommandResult {
//     let account = config
//         .rpc_client
//         .get_token_account(&address)
//         .map_err(|_| format!("Could not find token account {}", address))?
//         .unwrap();
//     let mint = Pubkey::from_str(&account.mint).unwrap();
//     let owner = Pubkey::from_str(&account.owner).unwrap();
//     let is_associated =
//         get_associated_token_address_with_program_id(&owner, &mint, &config.program_id) == address;
//     let cli_token_account = CliTokenAccount {
//         address: address.to_string(),
//         is_associated,
//         account,
//     };
//     Ok(config.output_format.formatted_string(&cli_token_account))
// }



struct SignOnlyNeedsFullMintSpec {}
impl offline::ArgsConfig for SignOnlyNeedsFullMintSpec {
    fn sign_only_arg<'a, 'b>(&self, arg: Arg<'a, 'b>) -> Arg<'a, 'b> {
        arg.requires_all(&[MINT_ADDRESS_ARG.name, MINT_DECIMALS_ARG.name])
    }
}

struct SignOnlyNeedsMintDecimals {}
impl offline::ArgsConfig for SignOnlyNeedsMintDecimals {
    fn sign_only_arg<'a, 'b>(&self, arg: Arg<'a, 'b>) -> Arg<'a, 'b> {
        arg.requires_all(&[MINT_DECIMALS_ARG.name])
    }
}

struct SignOnlyNeedsMintAddress {}
impl offline::ArgsConfig for SignOnlyNeedsMintAddress {
    fn sign_only_arg<'a, 'b>(&self, arg: Arg<'a, 'b>) -> Arg<'a, 'b> {
        arg.requires_all(&[MINT_ADDRESS_ARG.name])
    }
}

struct SignOnlyNeedsDelegateAddress {}
impl offline::ArgsConfig for SignOnlyNeedsDelegateAddress {
    fn sign_only_arg<'a, 'b>(&self, arg: Arg<'a, 'b>) -> Arg<'a, 'b> {
        arg.requires_all(&[DELEGATE_ADDRESS_ARG.name])
    }
}

fn main() -> Result<(), Error> {
    // let default_decimals = &format!("{}", native_mint::DECIMALS);
    let default_program_id = ppl_nft::id().to_string();
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
        // .bench_subcommand()
        .subcommand(SubCommand::with_name(CREATE_TOKEN).about("Create a new token")
                .arg(
                    Arg::with_name("token_keypair")
                        .value_name("TOKEN_KEYPAIR")
                        .validator(is_valid_signer)
                        .takes_value(true)
                        .index(1)
                        .help(
                            "Specify the token keypair. \
                             This may be a keypair file or the ASK keyword. \
                             [default: randomly generated keypair]"
                        ),
                )
                .arg(
                    Arg::with_name("mint_authority")
                        .long("mint-authority")
                        .alias("owner")
                        .value_name("ADDRESS")
                        .validator(is_valid_pubkey)
                        .takes_value(true)
                        .help(
                            "Specify the mint authority address. \
                             Defaults to the client keypair address."
                        ),
                )
                .arg(
                    Arg::with_name("total_supply")
                        .long("total-supply")
                        // .validator(is_mint_decimals)
                        .value_name("total_supply")
                        .takes_value(true)
                        .default_value("1")
                        .help("the nft's total supply to place"),
                )
                .arg(
                    Arg::with_name("enable_freeze")
                        .long("enable-freeze")
                        .takes_value(false)
                        .help(
                            "Enable the mint authority to freeze associated token accounts."
                        ),
                )
                .arg(
                    Arg::with_name("nft_name")
                        .long("nft-name")
                        .takes_value(true)
                        .help("the name of nft")
                )
                .arg(
                    Arg::with_name("nft_symbol")
                        .long("nft-symbol")
                        .takes_value(true)
                        .help("the symbol of nft")
                )
                .arg(
                    Arg::with_name("icon_uri")
                        .long("icon-uri")
                        .takes_value(true)
                        .help("the icon uri of nft")
                )
                .nonce_args(true)
                .arg(memo_arg())
                .offline_args(),
        )
        .subcommand(SubCommand::with_name("update")
                        .about("Update the nft account or nft mint account info. Check detail with -h.")
                        .arg(
                            Arg::with_name("address")
                                .value_name("address")
                                .takes_value(true)
                                .required(true)
                                .help("The address that uri will be update"),
                        )
                        .arg(
                            Arg::with_name("value")
                                .value_name("value")
                                .long("value")
                                .takes_value(true)
                                .required(true)
                                .help("The new value of update"),
                        )
                        .arg(
                            Arg::with_name("type")
                                .long("type")
                                // .validator(is_mint_supply)
                                .value_name("type")
                                .takes_value(true)
                                .required(true)
                                .help("The update type. Token mints support `icon` type;Token \
                                            accounts support `asset` type. [possible values: icon, asset]"),
                        )
        )
        .subcommand(
            SubCommand::with_name("mint")
                .about("mint a nft to a new account")
                .arg(
                    Arg::with_name("token")
                        .validator(is_valid_pubkey)
                        .long("token")
                        .value_name("token")
                        .takes_value(true)
                        .required(true)
                        .help("The token that the new NFT account will hold"),
                )
                .arg(
                    Arg::with_name("token_uri")
                        .long("token-uri")
                        .value_name("token-uri")
                        .takes_value(true)
                        .help(
                            "the uri of the nft"
                        ),
                )
                .arg(
                    Arg::with_name("owner_keypair")
                        .value_name("owner-keypair")
                        .long("owner-keypair")
                        .validator(is_valid_signer)
                        .takes_value(true)
                        .help(
                            "Specify the account keypair. \
                             This may be a keypair file or the ASK keyword. \
                             [default: associated token account for --owner]"
                        ),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("transfer")
                .about("transfer a nft to another wallet account")
                .arg(
                    Arg::with_name("nft")
                        .validator(is_valid_pubkey)
                        .long("nft")
                        .value_name("nft")
                        .takes_value(true)
                        .required(true)
                        .help("the nft to transfer"),
                )
                .arg(
                    Arg::with_name("from")
                        .long("from")
                        .validator(is_valid_signer)
                        .value_name("from")
                        .takes_value(true)
                        .required(true)
                        .help("the from account keypair \
                                This may be a keypair file or the ASK keyword.
                        "),
                )
                .arg(
                    Arg::with_name("recipient")
                        .long("recipient")
                        .value_name("recipient")
                        .validator(is_valid_pubkey)
                        .takes_value(true)
                        .required(true)
                        .help(
                            "the new owner of the account of the nft."
                        ),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("mint-info")
                .about("query info of mint")
                .arg(
                    Arg::with_name("address")
                        .validator(is_valid_pubkey)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address of mint"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("nft-info")
                .about("query info of special NFT")
                .arg(
                    Arg::with_name("address")
                        .validator(is_valid_pubkey)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address of mint of special NFT"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("mint-nfts")
                .about("query nfts of mint has been minted")
                .arg(
                    Arg::with_name("address")
                        .validator(is_valid_pubkey)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address of mint"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("nft-index-info")
                .about("query nft info by token id")
                .arg(
                    Arg::with_name("mint")
                        .long("mint")
                        .validator(is_valid_pubkey)
                        .value_name("mint")
                        .takes_value(true)
                        .required(true)
                        .help("the address of mint"),
                )
                .arg(
                    Arg::with_name("token-id")
                        .long("token-id")
                        .validator(is_mint_supply)
                        .value_name("token-id")
                        .takes_value(true)
                        .required(true)
                        .help("the token id of nft"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("multi-mint")
                .about("mint multi nft with same owner.")
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        .validator(is_valid_pubkey)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address of mint"),
                )
                .arg(
                    Arg::with_name("nfts")
                        .long("nfts")
                        // .validator(is_mint_supply)
                        .value_name("nfts")
                        .takes_value(true)
                        .required(true)
                        .help("the file of nfts uri"),
                )
                .arg(
                    Arg::with_name("owner_keypair")
                        .value_name("owner-keypair")
                        .long("owner-keypair")
                        .validator(is_valid_signer)
                        .takes_value(true)
                        .help(
                            "Specify the account keypair. \
                             This may be a keypair file or the ASK keyword. \
                             [default: associated token account for --owner]"
                        ),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("freeze")
                .about("Freeze a nft account.")
                .arg(
                    Arg::with_name("auth-keypair")
                        .long("auth-keypair")
                        .validator(is_valid_signer)
                        .value_name("auth-keypair")
                        .takes_value(true)
                        .help("the freeze authority keypair.default wallet keypair, if not set. \
                                This may be a keypair file or the ASK keyword.
                        "),
                )
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        // .validator(is_mint_supply)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address will be frozen"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("thaw")
                .about("Thaw a nft account.")
                .arg(
                    Arg::with_name("auth-keypair")
                        .long("auth-keypair")
                        .validator(is_valid_signer)
                        .value_name("auth-keypair")
                        .takes_value(true)
                        .help("the thaw authority keypair.default wallet keypair, if not set. \
                                This may be a keypair file or the ASK keyword.
                        "),
                )
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        // .validator(is_mint_supply)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address will be thaw"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("burn")
                .about("Burn a nft account.")
                .arg(
                    Arg::with_name("auth-keypair")
                        .long("auth-keypair")
                        .validator(is_valid_signer)
                        .value_name("auth-keypair")
                        .takes_value(true)
                        .help("the close authority keypair.default wallet keypair, if not set. \
                                This may be a keypair file or the ASK keyword.
                        "),
                )
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        // .validator(is_mint_supply)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address will be burn"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("authorize")
                .about("Authorize a new signing keypair to a token or token account.")
                .arg(
                    Arg::with_name("auth-keypair")
                        .long("auth-keypair")
                        .validator(is_valid_signer)
                        .value_name("auth-keypair")
                        .takes_value(true)
                        .help(" The address of the new authority"),
                )
                .arg(
                    Arg::with_name("address")
                        .long("address")
                        // .validator(is_mint_supply)
                        .value_name("address")
                        .takes_value(true)
                        .required(true)
                        .help("the address of a nft account or mint account."),
                )
                .arg(
                    Arg::with_name("type")
                        .long("type")
                        // .validator(is_mint_supply)
                        .value_name("type")
                        .takes_value(true)
                        .required(true)
                        .help("The new authority type. Token mints support `mint` and `freeze` authorities;Token \
                                accounts support `close` authorities. [possible values: mint, freeze, close]"),
                )
                .arg(owner_address_arg())
                .nonce_args(true)
                .offline_args(),
        )
        .subcommand(
            SubCommand::with_name("accounts")
                .about("List all token accounts by owner")
                .arg(
                    Arg::with_name("mint")
                        .validator(is_valid_pubkey)
                        .value_name("mint_address")
                        .takes_value(true)
                        .index(1)
                        .help("Limit results to the given mint address. [Default: list accounts for all mints]"),
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
        // ("bench", Some(arg_matches)) => bench_process_command(
        //     arg_matches,
        //     &config,
        //     std::mem::take(&mut bulk_signers),
        //     &mut wallet_manager,
        // ),
        (CREATE_TOKEN, Some(arg_matches)) => {
            let total_supply = value_t_or_exit!(arg_matches, "total_supply", u64);
            let name = value_t_or_exit!(arg_matches, "nft_name", String);
            let symbol = value_t_or_exit!(arg_matches, "nft_symbol", String);
            let icon_uri = value_t_or_exit!(arg_matches, "icon_uri", String);
            let mint_authority =
                config.pubkey_or_default(arg_matches, "mint_authority", &mut wallet_manager);
            let memo = value_t!(arg_matches, "memo", String).ok();

            let (token_signer, token) =
                get_signer(arg_matches, "token_keypair", &mut wallet_manager)
                    .unwrap_or_else(new_throwaway_signer);
            bulk_signers.push(token_signer);

            command_create_token(
                &config,
                total_supply,
                token,
                mint_authority,
                arg_matches.is_present("enable_freeze"),
                memo,
                name,
                symbol,
                icon_uri,
                bulk_signers,
            )
        }
        ("update", Some(arg_matches)) => {

            let value = value_t_or_exit!(arg_matches, "value", String);
            let address = value_t_or_exit!(arg_matches, "address", String);
            let update_type_str = value_t_or_exit!(arg_matches, "type", String);

            let address_pubkey = Pubkey::from_str(address.as_str()).unwrap();

            let update_type : UpdateType;
            match update_type_str.as_str() {
                "icon" => {
                    update_type = UpdateType::Icon {icon_uri: value}
                }
                "asset" => {
                    update_type = UpdateType::NftAsset {token_uri: value}
                }
                _ => {
                    return Err(Error::try_from("invalid update type.").unwrap())
                }
            }


            let (token_signer, _sender) =  config.signer_or_default(arg_matches,"_none", &mut wallet_manager);
            bulk_signers.push(token_signer);

            command_update(
                &config,
                update_type,
                address_pubkey,
                bulk_signers,
            )
        }
        ("mint", Some(arg_matches)) => {
            // let token = pubkey_of_signer(arg_matches, "token", &mut wallet_manager)
            //     .unwrap()
            //     .unwrap();
            let mint = value_t_or_exit!(arg_matches, "token", String);
            let mint_pubkey = Pubkey::from_str(mint.as_str()).unwrap();
            let token_uri = value_t_or_exit!(arg_matches, "token_uri", String);

            // No need to add a signer when creating an associated token account
            let mint_account = config.rpc_client.get_account(&mint_pubkey).unwrap();
            let mint = NftMint::unpack(mint_account.data()).unwrap();

            let (nft_token, _) = find_nft_pubkey(mint.supply + 1, config.program_id, mint_pubkey);
            // let (signer, nft_token) = get_signer(arg_matches, "account_keypair", &mut wallet_manager).unwrap_or_else(new_throwaway_signer);
            // bulk_signers.push(signer);
            let (owner_signer, owner) =  config.signer_or_default(arg_matches,"owner_keypair", &mut wallet_manager);
            bulk_signers.push(owner_signer);

            // let owner = config.pubkey_or_default(arg_matches, "owner", &mut wallet_manager);
            command_mint(
                &config,
                mint_pubkey,
                nft_token,
                owner,
                token_uri,
                bulk_signers
            )
        }
        ("transfer", Some(arg_matches)) => {
            let nft = value_t_or_exit!(arg_matches, "nft", String);
            let nft_pubkey = Pubkey::from_str(nft.as_str()).unwrap();

            let _from = value_t_or_exit!(arg_matches, "from", String);


            let (sender_signer, sender) = config.signer_or_default(arg_matches, "from", &mut wallet_manager);

            let recipient = value_t_or_exit!(arg_matches, "recipient", String);
            let recipient_pubkey = Pubkey::from_str(recipient.as_str()).unwrap();

            bulk_signers.push(sender_signer);


            command_transfer(
                &config,
                sender,
                recipient_pubkey,
                nft_pubkey,
                bulk_signers
            )
        }
        ("mint-info", Some(arg_matches)) => {
            let mint_address = value_t_or_exit!(arg_matches, "address", String);
            let mint_pubkey = Pubkey::from_str(mint_address.as_str()).unwrap();

            let mint_account = config.rpc_client.get_account(&mint_pubkey)?;
            let mint = NftMint::unpack(mint_account.data()).expect(&*format!("Could not find NFT mint account {}", mint_address));
            let ui_mint_info = UiNftMintInfo {
                mint_authority: mint.mint_authority,
                supply: mint.supply,
                total_supply:  mint.total_supply,
                is_initialized: mint.is_initialized,
                name: mint.name,
                symbol: mint.symbol,
                freeze_authority: mint.freeze_authority,
                icon_uri: mint.icon_uri
            };
            let cli_display_mint = CliDisplayMint {
                address: mint_address.to_string(),
                account: ui_mint_info,
            };
            Ok(config.output_format.formatted_string(&cli_display_mint))
        }
        ("nft-info", Some(arg_matches)) => {
            let nft_address = value_t_or_exit!(arg_matches, "address", String);
            let nft_pubkey = Pubkey::from_str(nft_address.as_str()).unwrap();

            let nft_account = config.rpc_client.get_account(&nft_pubkey)?;
            let meta = MetaAccount::unpack(nft_account.data()).expect(&*format!("Could not find NFT account {}", nft_address));

            let mint_account = config.rpc_client.get_account(&meta.mint)?;
            let mint_account_obj = NftMint::unpack(mint_account.data()).expect(&*format!("Could not find NFT account {}", nft_address));

            let ui_nft_info = UiNftInfo {
                mint: meta.mint,
                owner: meta.owner,
                state: meta.state.to_string(),
                close_authority: meta.close_authority,
                token_id: meta.token_id,
                token_uri: meta.token_uri,
                name: mint_account_obj.name,
                symbol: mint_account_obj.symbol
            };
            let cli_display_nft_info = CliDisplayNftInfo {
                address: nft_address.to_string(),
                account: ui_nft_info,
            };
            Ok(config.output_format.formatted_string(&cli_display_nft_info))
        }
        ("mint-nfts", Some(arg_matches)) => {
            let mint_address = value_t_or_exit!(arg_matches, "address", String);
            let mint_pubkey = Pubkey::from_str(mint_address.as_str()).unwrap();

            let mint_account = config.rpc_client.get_account(&mint_pubkey)?;
            let mint = NftMint::unpack(mint_account.data()).expect(&*format!("Could not find NFT mint account {}", mint_address));

            let mut nft_list: Vec<String> = Vec::new();

            // Find minted nfts
            for i in 1..=mint.supply {
                let index = i.to_le_bytes();
                let signer_seeds = &[
                    index.as_slice(),
                    config.program_id.as_ref(),
                    mint_pubkey.as_ref(),
                ];
                let (nft_token, _) =
                    Pubkey::find_program_address(signer_seeds, &config.program_id);
                nft_list.push(nft_token.to_string())
            }
            if nft_list.is_empty() {
                println!("{} have not minted nft ", mint_address);
                return Ok(())
            }
            println!("-------------------- nft list --------------------");
            for nft in nft_list {
                println!("{}", nft);
            }
            Ok("".to_string())
        }
        ("nft-index-info", Some(arg_matches)) => {
            let mint_address = value_t_or_exit!(arg_matches, "mint", String);
            let token_id = value_t_or_exit!(arg_matches, "token-id", u64);
            let mint_pubkey = Pubkey::from_str(mint_address.as_str()).unwrap();

            let _ = config.rpc_client.get_account(&mint_pubkey)?;

            let (nft_token, _) = find_nft_pubkey(token_id, config.program_id, mint_pubkey);

            let nft_account = config.rpc_client.get_account(&nft_token).expect(format!("token id {} have not mint", token_id).as_str());
            let meta = MetaAccount::unpack(nft_account.data()).expect(&*format!("Could not find NFT account by token {}", nft_token));

            let mint_account = config.rpc_client.get_account(&meta.mint)?;
            let mint_account_obj = NftMint::unpack(mint_account.data()).expect(&*format!("Could not find NFT account {}", nft_token));

            println!();
            println!("Mint {} at index {}'s Nft is: \t  {} ", mint_address, token_id, nft_token);

            println!("The nft {} owner is: \t  {}", nft_token, meta.owner);
            let ui_nft_info = UiNftInfo {
                mint: meta.mint,
                owner: meta.owner,
                state: meta.state.to_string(),
                close_authority: meta.close_authority,
                token_id: meta.token_id,
                token_uri: meta.token_uri,
                name: mint_account_obj.name,
                symbol: mint_account_obj.symbol
            };
            let cli_display_nft_info = CliDisplayNftInfo {
                address: nft_token.to_string(),
                account: ui_nft_info,
            };
            Ok(config.output_format.formatted_string(&cli_display_nft_info))
        }
        ("multi-mint", Some(arg_matches)) => {
            let mint_address = value_t_or_exit!(arg_matches, "address", String);
            let path = value_t_or_exit!(arg_matches, "nfts", String);

            // read urls list
            let mut uri_list: Vec<String> = Vec::new();
            let file = File::open(path).unwrap();
            let line = io::BufReader::new(file).lines();

            for line in line {
                if let Ok(uri) = line {
                    uri_list.push(uri);
                }
            }
            // Get current mint
            let mint_pubkey = Pubkey::from_str(mint_address.as_str()).unwrap();

            let mint_account = config.rpc_client.get_account(&mint_pubkey)?;
            let mint = NftMint::unpack(mint_account.data()).expect(&*format!("Could not find NFT mint account {}", mint_address));
            if mint.total_supply - mint.supply < uri_list.len() as u64 {
                return Err(Error::try_from("the nft wish to mint greater than max supply").unwrap())
            }

            // Start to mint
            for i in 1..= uri_list.len() {
                // Signer not implement clone trait, bulk_signers cannot repeat use,
                // So built new bulk_signers for one cycle
                let mut bulk_signers: Vec<Box<dyn Signer>> = Vec::new();
                let (owner_signer, owner) =  config.signer_or_default(arg_matches,"owner_keypair", &mut wallet_manager);
                bulk_signers.push(owner_signer);

                let (nft_token, _) = find_nft_pubkey(mint.supply + (i as u64), config.program_id, mint_pubkey);

                let uri = uri_list.get(i - 1).unwrap().deref();
                // let owner = config.pubkey_or_default(arg_matches, "owner", &mut wallet_manager);
                command_mint(
                    &config,
                    mint_pubkey,
                    nft_token,
                    owner,
                    uri.to_string(),
                    bulk_signers
                )?;
                println!("\tminted nft {}", nft_token);
            }
            Ok("".to_string())
        }
        ("freeze", Some(arg_matches)) => {
            let nft_address = value_t_or_exit!(arg_matches, "address", String);

            let token_pubkey = Pubkey::from_str(nft_address.as_str()).unwrap();
            let nft_account = config.rpc_client.get_account(&token_pubkey).expect(format!("nft {} not found", nft_address).as_str());
            let meta = MetaAccount::unpack(nft_account.data()).expect(&*format!("invalid nft token {}", token_pubkey));


            let (sender_signer, sender) =
                config.signer_or_default(arg_matches, "auth-keypair", &mut wallet_manager);

            bulk_signers.push(sender_signer);

            command_freeze(
                &config,
                meta.mint,
                token_pubkey,
                sender,
                bulk_signers
            )
        }
        ("thaw", Some(arg_matches)) => {
            let nft_address = value_t_or_exit!(arg_matches, "address", String);
            // let auth_path = value_t!(arg_matches, "auth-keypair", String);


            let token_pubkey = Pubkey::from_str(nft_address.as_str()).unwrap();
            let nft_account = config.rpc_client.get_account(&token_pubkey).expect(format!("nft {} not found", nft_address).as_str());
            let meta = MetaAccount::unpack(nft_account.data()).expect(&*format!("invalid nft token {}", token_pubkey));


            let (sender_signer, sender) =
                config.signer_or_default(arg_matches, "auth-keypair", &mut wallet_manager);

            bulk_signers.push(sender_signer);

            command_thaw(
                &config,
                meta.mint,
                token_pubkey,
                sender,
                bulk_signers
            )
        }
        ("burn", Some(arg_matches)) => {
            let nft_address = value_t_or_exit!(arg_matches, "address", String);
            // let auth_path = value_t!(arg_matches, "auth-keypair", String);


            let token_pubkey = Pubkey::from_str(nft_address.as_str()).unwrap();
            let nft_account = config.rpc_client.get_account(&token_pubkey).expect(format!("nft {} not found", nft_address).as_str());
            let meta = MetaAccount::unpack(nft_account.data()).expect(&*format!("invalid nft token {}", token_pubkey));
            if meta.is_frozen() {
                return Err(Error::try_from("the nft has been frozen").unwrap())
            }

            let (sender_signer, sender) =
                config.signer_or_default(arg_matches, "auth-keypair", &mut wallet_manager);

            bulk_signers.push(sender_signer);

            command_burn(
                &config,
                token_pubkey,
                sender,
                bulk_signers
            )
        }
        ("authorize", Some(arg_matches)) => {
            let address = value_t_or_exit!(arg_matches, "address", String);
            let auth_path = value_t!(arg_matches, "auth-keypair", String);
            let auth_type = value_t_or_exit!(arg_matches, "type", String);
            // Change authority type
            let authority_type : AuthorityType;
            match auth_type.as_str() {
                "mint" => {
                    authority_type = AuthorityType::MintTokens;
                }
                "freeze" => {
                    authority_type = AuthorityType::FreezeAccount;
                }
                "close" => {
                    authority_type = AuthorityType::CloseAccount;
                }
                _ => {
                    return Err(Error::try_from("invalid authority type.").unwrap())
                }
            }
            // Read new authority signer
            let mut new_authority = None;
            if auth_path.is_ok() {
                let (sender_signer, sender) =
                    config.signer_or_default(arg_matches, "auth-keypair", &mut wallet_manager);
                bulk_signers.push(sender_signer);
                println!("sender {}", sender.to_string());
                new_authority = Some(sender);
            }

            // Read old owner
            let (sender_signer, sender) =
                config.signer_or_default(arg_matches, "owner", &mut wallet_manager);
            bulk_signers.push(sender_signer);

            let address = Pubkey::from_str(address.as_str()).unwrap();

            command_authorize(
                &config,
                address,
                new_authority,
                authority_type,
                sender,
                bulk_signers
            )
        }
        ("accounts", Some(arg_matches)) => {
            let mint = pubkey_of_signer(arg_matches, "mint", &mut wallet_manager).unwrap();
            let owner = config.pubkey_or_default(arg_matches, "owner", &mut wallet_manager);
            command_accounts(&config, mint, owner)
        }
        _ => unreachable!(),
    }
    .map_err::<Error, _>(|err| DisplayError::new_as_boxed(err).into())?;
    println!("{}", result);
    Ok(())
}

fn find_nft_pubkey(index :u64, program_id: Pubkey, mint_key: Pubkey) -> (Pubkey, u8)  {
    let index = index.to_le_bytes();
    let signer_seeds = &[
        index.as_slice(),
        program_id.as_ref(),
        mint_key.as_ref(),
    ];
    Pubkey::find_program_address(signer_seeds, &program_id)
}

fn validate_mint(config: &Config, token: Pubkey) -> CommandResult {
    let mint_account = config.rpc_client.get_account(&token)?;
    let _ = NftMint::unpack(&mint_account.data)?;
    Ok("".to_string())
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
