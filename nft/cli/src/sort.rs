use std::collections::btree_map::{Entry, BTreeMap};
use put_account_decoder::parse_nft::{TokenAccountType};
use put_account_decoder::UiAccountData;
use put_client::rpc_response::RpcKeyedAccount;
use serde::{Deserialize, Serialize};
use crate::output::CliNftAccount;

pub(crate) type MintAccounts = BTreeMap<String, Vec<CliNftAccount>>;

// pub struct ParsedTokenAccount {
//     pub address: String,
//     pub ui_token_account: UiNFTAccount,
// }


#[derive(Serialize, Deserialize)]
pub(crate) struct UnsupportedAccount {
    pub address: String,
    pub err: String,
}

pub(crate) fn sort_and_parse_token_accounts(
    accounts: Vec<RpcKeyedAccount>,
) -> (MintAccounts, Vec<UnsupportedAccount>, usize) {
    let mut mint_accounts: MintAccounts = BTreeMap::new();
    let mut unsupported_accounts = vec![];
    let mut max_token_id_len = 0;
    for keyed_account in accounts {
        let address = keyed_account.pubkey;

        if let UiAccountData::Json(parsed_account) = keyed_account.account.data {
            if parsed_account.program != "ppl-nft" {
                unsupported_accounts.push(UnsupportedAccount {
                    address,
                    err: format!("Unsupported account program: {}", parsed_account.program),
                });
            } else {
                match serde_json::from_value(parsed_account.parsed) {
                    Ok(TokenAccountType::Account(ui_token_account)) => {
                        let mint = ui_token_account.mint.clone();

                        let parsed_account = CliNftAccount {
                            address,
                            account: ui_token_account
                        };
                        let token_id_len = parsed_account.account
                            .token_id
                            .to_string()
                            .len();
                        max_token_id_len = max_token_id_len.max(token_id_len);

                        let entry = mint_accounts.entry(mint);
                        match entry {
                            Entry::Occupied(_) => {
                                entry.and_modify(|e| e.push(parsed_account));
                            }
                            Entry::Vacant(_) => {
                                entry.or_insert_with(|| vec![parsed_account]);
                            }
                        }
                    }
                    Ok(_) => unsupported_accounts.push(UnsupportedAccount {
                        address,
                        err: "Not a NFT account".to_string(),
                    }),
                    Err(err) => unsupported_accounts.push(UnsupportedAccount {
                        address,
                        err: format!("Account parse failure: {}", err),
                    }),
                }
            }
        } else {
            unsupported_accounts.push(UnsupportedAccount {
                address,
                err: "Unsupported account data format".to_string(),
            });
        }
    }
    (
        mint_accounts,
        unsupported_accounts,
        max_token_id_len
    )
}