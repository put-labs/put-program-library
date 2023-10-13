use crate::{config::Config, sort::UnsupportedAccount};
use console::Emoji;
use serde::{Deserialize, Serialize, Serializer};
use put_cli_output::{display::writeln_name_value, OutputFormat, QuietDisplay, VerboseDisplay};
use std::fmt::{self, Display};
use put_account_decoder::parse_nft::{UiNFTAccount, UiAccountState};
use put_sdk::pubkey::Pubkey;

pub(crate) trait Output: Serialize + fmt::Display + QuietDisplay + VerboseDisplay {}

static WARNING: Emoji = Emoji("⚠️", "!");

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandOutput<T>
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) command_name: String,
    pub(crate) command_output: T,
}

impl<T> Display for CommandOutput<T>
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.command_output, f)
    }
}

impl<T> QuietDisplay for CommandOutput<T>
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        QuietDisplay::write_str(&self.command_output, w)
    }
}

impl<T> VerboseDisplay for CommandOutput<T>
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln_name_value(w, "Command: ", &self.command_name)?;
        VerboseDisplay::write_str(&self.command_output, w)
    }
}

pub(crate) fn println_display(config: &Config, message: String) {
    match config.output_format {
        OutputFormat::Display | OutputFormat::DisplayVerbose => {
            println!("{}", message);
        }
        _ => {}
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliCreateMint<T>
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) address: String,
    pub(crate) total_supply: u64,
    pub(crate) icon_uri : String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliCreateMint<T>
where
    T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "Address: ", &self.address)?;
        writeln_name_value(f, "total_supply: ", &format!("{}", self.total_supply))?;
        writeln_name_value(f, "icon_uri: ", &format!("{}", self.icon_uri))?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliCreateMint<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "Address: ", &self.address)?;
        writeln_name_value(w, "total_supply: ", &format!("{}", self.total_supply))?;
        writeln_name_value(w, "icon_uri: ", &format!("{}", self.icon_uri))?;
        QuietDisplay::write_str(&self.transaction_data, w)
    }
}
impl<T> VerboseDisplay for CliCreateMint<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "Address: ", &self.address)?;
        writeln_name_value(w, "total_supply: ", &format!("{}", self.total_supply))?;
        writeln_name_value(w, "icon_uri: ", &format!("{}", self.icon_uri))?;
        VerboseDisplay::write_str(&self.transaction_data, w)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliMintTo<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) address: String,
    pub(crate) token_uri: String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliMintTo<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "Address: ", &self.address)?;
        writeln_name_value(f, "token_uri: ", &self.token_uri)?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliMintTo<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "Address: ", &self.address)?;
        writeln_name_value(w, "token_uri: ", &self.token_uri)?;
        QuietDisplay::write_str(&self.transaction_data, w)
    }
}
impl<T> VerboseDisplay for CliMintTo<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "Address: ", &self.address)?;
        writeln_name_value(w, "token_uri: ", &self.token_uri)?;
        VerboseDisplay::write_str(&self.transaction_data, w)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliAuthorize<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) authorize_account: String,
    pub(crate) new_authority: Option<Pubkey>,
    pub(crate) auth_type: String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliAuthorize<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        let mut new_authority = "None".to_string();
        if self.new_authority.is_some() {
            new_authority = self.new_authority.unwrap().to_string();
        }
        writeln!(f, "address {} {} authority change to {}: ", &self.authorize_account, &self.auth_type, new_authority)?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliAuthorize<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        let mut new_authority = "None".to_string();
        if self.new_authority.is_some() {
            new_authority = self.new_authority.unwrap().to_string();
        }
        writeln!(w, "address {} {} authority change to {}: ", &self.authorize_account, &self.auth_type, new_authority)?;
        QuietDisplay::write_str(&self.transaction_data, w)
    }
}
impl<T> VerboseDisplay for CliAuthorize<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        let mut new_authority = "None".to_string();
        if self.new_authority.is_some() {
            new_authority = self.new_authority.unwrap().to_string();
        }
        writeln!(w, "address {} {} authority change to {}: ", &self.authorize_account, &self.auth_type, new_authority)?;
        VerboseDisplay::write_str(&self.transaction_data, w)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliBurn<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) burn_nft: String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliBurn<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "burn nft: ", &self.burn_nft)?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliBurn<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "burn nft: ", &self.burn_nft)?;
        QuietDisplay::write_str(&self.transaction_data, w)
    }
}
impl<T> VerboseDisplay for CliBurn<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "burn nft: ", &self.burn_nft)?;
        VerboseDisplay::write_str(&self.transaction_data, w)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliThaw<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) thaw_nft: String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliThaw<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "thaw nft: ", &self.thaw_nft)?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliThaw<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{}
impl<T> VerboseDisplay for CliThaw<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliFreeze<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) freeze_nft: String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliFreeze<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "freeze nft: ", &self.freeze_nft)?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliFreeze<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{}
impl<T> VerboseDisplay for CliFreeze<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{}


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliTransfer<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) nft: String,
    pub(crate) transaction_data: T,
}

impl<T> Display for CliTransfer<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "sender: ", &self.from)?;
        writeln_name_value(f, "recipient: ", &self.to)?;
        writeln_name_value(f, "nft: ", &self.nft)?;
        Display::fmt(&self.transaction_data, f)
    }
}

impl<T> QuietDisplay for CliTransfer<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "sender: ", &self.from)?;
        writeln_name_value(w, "recipient: ", &self.to)?;
        writeln_name_value(w, "nft: ", &self.nft)?;
        QuietDisplay::write_str(&self.transaction_data, w)
    }
}
impl<T> VerboseDisplay for CliTransfer<T>
    where
        T: Serialize + Display + QuietDisplay + VerboseDisplay,
{
    fn write_str(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w)?;
        writeln_name_value(w, "sender: ", &self.from)?;
        writeln_name_value(w, "recipient: ", &self.to)?;
        writeln_name_value(w, "nft: ", &self.nft)?;
        VerboseDisplay::write_str(&self.transaction_data, w)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UiNftMintInfo {
    /// Optional authority used to mint new tokens. The mint authority may only be provided during
    /// mint creation. If no mint authority is present then the mint has a fixed supply and no
    /// further tokens may be minted.
    pub mint_authority: Pubkey, //32
    /// supply of tokens in now.
    pub supply: u64,//8
    /// supply of tokens in total.
    pub total_supply: u64,//8
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,//1
    /// name of nfts
    pub name: String,
    /// symbol of nfts
    pub symbol: String,
    /// Optional authority to freeze token accounts.
    pub freeze_authority: Option<Pubkey>, //36
    /// icon uri of nft
    pub icon_uri: String, //36
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliDisplayMint {
    pub(crate) address: String,
    // #[serde(flatten)]
    pub(crate) account: UiNftMintInfo,
}

impl QuietDisplay for CliDisplayMint {}
impl VerboseDisplay for CliDisplayMint {}

impl fmt::Display for CliDisplayMint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(
            f,
            "Already Supply:",
            &self.account.supply.to_string()
        )?;
        writeln_name_value(
            f,
            "Total Supply:",
            &self.account.total_supply.to_string()
        )?;
        let mint = format!(
            "{}",
            self.address,
        );
        let mut freeze_authority = "None".to_string();
        if self.account.freeze_authority.is_some() {
            freeze_authority = self.account.freeze_authority.unwrap().to_string();
        }
        writeln_name_value(f, "Mint:", &mint)?;
        writeln_name_value(f, "Name:", &self.account.name)?;
        writeln_name_value(f, "Symbol:", &self.account.symbol)?;
        writeln_name_value(f, "Icon uri:", &self.account.icon_uri)?;
        writeln_name_value(f, "Mint authority:", &self.account.mint_authority.to_string())?;
        writeln_name_value(f, "Freeze authority:", &freeze_authority)?;
        writeln_name_value(f, "Initialized:", &format!("{:?}", self.account.is_initialized))?;

        Ok(())
    }
}


#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UiNftInfo {
    /// The mint associated with this account
    pub mint: Pubkey, //32
    /// The owner of this account.
    pub owner: Pubkey, //32
    /// The account's state
    pub state: String, //1
    /// Optional authority to close the account
    pub close_authority: Option<Pubkey>,// 33
    /// The mint's token_id of nft
    pub token_id: u64, // 8
    /// The suffix of the nft
    pub token_uri: String, // 200
    /// the nft mint name
    pub name: String,
    /// the nft mint symbol
    pub symbol: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliDisplayNftInfo {
    pub(crate) address: String,
    // #[serde(flatten)]
    pub(crate) account: UiNftInfo,
}

impl QuietDisplay for CliDisplayNftInfo {}
impl VerboseDisplay for CliDisplayNftInfo {}

impl fmt::Display for CliDisplayNftInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f)?;
        let mut close_authority = "None".to_string();
        if self.account.close_authority.is_some() {
            close_authority = self.account.close_authority.unwrap().to_string();
        }

        writeln_name_value(f, "Mint:", &self.account.mint.to_string())?;
        writeln_name_value(f, "Mint name:", &self.account.name)?;
        writeln_name_value(f, "Mint symbol:", &self.account.symbol)?;
        writeln_name_value(f, "Owner:", &self.account.owner.to_string())?;
        writeln_name_value(f, "State:", &self.account.state)?;
        writeln_name_value(f, "Token id:", &self.account.token_id.to_string())?;
        writeln_name_value(f, "Token uri:", &self.account.token_uri)?;
        writeln_name_value(f, "Close authority:", &close_authority)?;
        Ok(())
    }
}




#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliWalletAddress {
    pub(crate) wallet_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) associated_token_address: Option<String>,
}

impl QuietDisplay for CliWalletAddress {}
impl VerboseDisplay for CliWalletAddress {}

impl fmt::Display for CliWalletAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Wallet address: {}", self.wallet_address)?;
        if let Some(associated_token_address) = &self.associated_token_address {
            writeln!(f, "Associated token address: {}", associated_token_address)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliMultisig {
    pub(crate) address: String,
    pub(crate) m: u8,
    pub(crate) n: u8,
    pub(crate) signers: Vec<String>,
}

impl QuietDisplay for CliMultisig {}
impl VerboseDisplay for CliMultisig {}

impl fmt::Display for CliMultisig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(f, "Address:", &self.address)?;
        writeln_name_value(f, "M/N:", &format!("{}/{}", self.m, self.n))?;
        writeln_name_value(f, "Signers:", " ")?;
        let width = if self.n >= 9 { 4 } else { 3 };
        for i in 0..self.n as usize {
            let title = format!("{1:>0$}:", width, i + 1);
            let pubkey = &self.signers[i];
            writeln_name_value(f, &title, pubkey)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliNftAccount {
    pub(crate) address: String,
    // pub(crate) is_associated: bool,
    #[serde(flatten)]
    pub(crate) account: UiNFTAccount,
}

impl QuietDisplay for CliNftAccount {}
impl VerboseDisplay for CliNftAccount {}

impl fmt::Display for CliNftAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f)?;
        writeln_name_value(
            f,
            "TokenId:",
            &self.account.token_id.to_string(),
        )?;
        let mint = format!(
            "{}",
            self.account.mint,
        );
        writeln_name_value(f, "Mint:", &mint)?;
        writeln_name_value(f, "Owner:", &self.account.owner)?;
        writeln_name_value(f, "State:", &format!("{:?}", self.account.state))?;

        writeln_name_value(
            f,
            "Close authority:",
            self.account
                .close_authority
                .as_ref()
                .unwrap_or(&String::new()),
        )?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliNftAccounts {
    #[serde(serialize_with = "flattened")]
    pub(crate) accounts: Vec<Vec<CliNftAccount>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) unsupported_accounts: Vec<UnsupportedAccount>,
    #[serde(skip_serializing)]
    pub(crate) max_token_id_len: usize,
    // #[serde(skip_serializing)]
    // pub(crate) aux_len: usize,
    #[serde(skip_serializing)]
    pub(crate) token_is_some: bool,
}

impl QuietDisplay for CliNftAccounts {}
impl VerboseDisplay for CliNftAccounts {
    fn write_str(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        if self.token_is_some {
            writeln!(
                w,
                "{:<44}  {:<2$}",
                "Account", "TokenId", self.max_token_id_len
            )?;
            writeln!(
                w,
                "-------------------------------------------------------------"
            )?;
        } else {
            writeln!(
                w,
                "{:<44}  {:<44}  {:<3$}",
                "Mint", "Account", "TokenId", self.max_token_id_len
            )?;
            writeln!(w, "----------------------------------------------------------------------------------------------------------")?;
        }
        for accounts_list in self.accounts.iter() {
            for account in accounts_list {
                let maybe_frozen = if let UiAccountState::Frozen = account.account.state {
                    format!(" {}  Frozen", WARNING)
                } else {
                    "".to_string()
                };
                if self.token_is_some {
                    writeln!(
                        w,
                        "{:<44}  {:<3$}{}",
                        account.address,
                        account.account.token_id.to_string(),
                        maybe_frozen,
                        self.max_token_id_len,
                    )?;
                } else {
                    writeln!(
                        w,
                        "{:<44}  {:<44}  {:<4$}{}",
                        account.account.mint,
                        account.address,
                        account.account.token_id.to_string(),
                        maybe_frozen,
                        self.max_token_id_len,
                    )?;
                }
            }
        }
        for unsupported_account in &self.unsupported_accounts {
            writeln!(
                w,
                "{:<44}  {}",
                unsupported_account.address, unsupported_account.err
            )?;
        }
        Ok(())
    }
}

impl fmt::Display for CliNftAccounts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let gc_alert = false;
        if self.token_is_some {
            writeln!(f, "{:<1$}", "TokenId", self.max_token_id_len)?;
            writeln!(f, "-------------")?;
        } else {
            writeln!(
                f,
                "{:<44}  {:<2$}",
                "Mint", "TokenId", self.max_token_id_len
            )?;
            writeln!(
                f,
                "---------------------------------------------------------------"
            )?;
        }
        for accounts_list in self.accounts.iter() {
            for account in accounts_list {
                let maybe_frozen = if let UiAccountState::Frozen = account.account.state {
                    format!(" {}  Frozen", WARNING)
                } else {
                    "".to_string()
                };
                if self.token_is_some {
                    writeln!(
                        f,
                        "{:<2$}{}",
                        account.account.token_id.to_string(),
                        maybe_frozen,
                        self.max_token_id_len,
                    )?;
                } else {
                    writeln!(
                        f,
                        "{:<44}  {:<3$}{}",
                        account.account.mint,
                        account.account.token_id.to_string(),
                        maybe_frozen,
                        self.max_token_id_len,
                    )?;
                }
            }
        }
        for unsupported_account in &self.unsupported_accounts {
            writeln!(
                f,
                "{:<44}  {}",
                unsupported_account.address, unsupported_account.err
            )?;
        }
        if gc_alert {
            writeln!(f)?;
            writeln!(f, "* Please run `spl-token gc` to clean up Aux accounts")?;
        }
        Ok(())
    }
}

fn flattened<S: Serializer>(
    vec: &[Vec<CliNftAccount>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let flattened: Vec<_> = vec.iter().flatten().collect();
    flattened.serialize(serializer)
}
