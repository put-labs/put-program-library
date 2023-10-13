use crate::{config::Config};
use serde::{Deserialize, Serialize};
use put_cli_output::{display::writeln_name_value, OutputFormat, QuietDisplay, VerboseDisplay};
use std::fmt::{self, Display};

pub(crate) trait Output: Serialize + fmt::Display + QuietDisplay + VerboseDisplay {}


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
