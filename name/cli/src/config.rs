use clap::ArgMatches;
use put_clap_utils::{
    keypair::{signer_from_path_with_config, SignerFromPathConfig},
};
use put_cli_output::OutputFormat;
use put_client::{blockhash_query::BlockhashQuery, rpc_client::RpcClient};
use put_remote_wallet::remote_wallet::RemoteWalletManager;
use put_sdk::{pubkey::Pubkey, signature::Signer};
use std::{process::exit, sync::Arc};
use put_clap_utils::input_parsers::pubkey_of_signer;
use put_clap_utils::keypair::pubkey_from_path;

pub(crate) struct Config<'a> {
    pub(crate) rpc_client: Arc<RpcClient>,
    pub(crate) _websocket_url: String,
    pub(crate) output_format: OutputFormat,
    pub(crate) fee_payer: Pubkey,
    pub(crate) default_keypair_path: String,
    pub(crate) nonce_account: Option<Pubkey>,
    pub(crate) nonce_authority: Option<Pubkey>,
    pub(crate) blockhash_query: BlockhashQuery,
    pub(crate) sign_only: bool,
    pub(crate) dump_transaction_message: bool,
    pub(crate) multisigner_pubkeys: Vec<&'a Pubkey>,
    pub(crate) program_id: Pubkey,
}

impl<'a> Config<'a> {

    // Checks if an explicit address was provided, otherwise return the default address.
    pub(crate) fn pubkey_or_default(
        &self,
        arg_matches: &ArgMatches,
        address_name: &str,
        wallet_manager: &mut Option<Arc<RemoteWalletManager>>,
    ) -> Pubkey {
        if address_name != "owner" {
            if let Some(address) =
            pubkey_of_signer(arg_matches, address_name, wallet_manager).unwrap()
            {
                return address;
            }
        }

        return self
            .default_address(arg_matches, wallet_manager)
            .unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                exit(1);
            });
    }

    fn default_address(
        &self,
        matches: &ArgMatches,
        wallet_manager: &mut Option<Arc<RemoteWalletManager>>,
    ) -> Result<Pubkey, Box<dyn std::error::Error>> {
        // for backwards compatibility, check owner before cli config default
        if let Some(address) = pubkey_of_signer(matches, "owner", wallet_manager).unwrap() {
            return Ok(address);
        }

        let path = &self.default_keypair_path;
        pubkey_from_path(matches, path, "default", wallet_manager)
    }

    // Checks if an explicit signer was provided, otherwise return the default signer.
    pub(crate) fn signer_or_default(
        &self,
        arg_matches: &ArgMatches,
        authority_name: &str,
        wallet_manager: &mut Option<Arc<RemoteWalletManager>>,
    ) -> (Box<dyn Signer>, Pubkey) {
        // If there are `--multisig-signers` on the command line, allow `NullSigner`s to
        // be returned for multisig account addresses
        let config = SignerFromPathConfig {
            allow_null_signer: !self.multisigner_pubkeys.is_empty(),
        };
        let mut load_authority = move || {
            // fallback handled in default_signer() for backward compatibility
            if authority_name != "owner" {
                if let Some(keypair_path) = arg_matches.value_of(authority_name) {
                    return signer_from_path_with_config(
                        arg_matches,
                        keypair_path,
                        authority_name,
                        wallet_manager,
                        &config,
                    );
                }
            }

            self.default_signer(arg_matches, wallet_manager, &config)
        };

        let authority = load_authority().unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });

        let authority_address = authority.pubkey();
        (authority, authority_address)
    }

    fn default_signer(
        &self,
        matches: &ArgMatches,
        wallet_manager: &mut Option<Arc<RemoteWalletManager>>,
        config: &SignerFromPathConfig,
    ) -> Result<Box<dyn Signer>, Box<dyn std::error::Error>> {
        // for backwards compatibility, check owner before cli config default
        if let Some(owner_path) = matches.value_of("owner") {
            return signer_from_path_with_config(
                matches,
                owner_path,
                "owner",
                wallet_manager,
                config,
            );
        }

        let path = &self.default_keypair_path;
        signer_from_path_with_config(matches, path, "default", wallet_manager, config)
    }
}
