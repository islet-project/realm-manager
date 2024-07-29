use clap::Parser;

use crate::commands::Command;

#[derive(Parser)]
#[command(version, about, long_about = None, multicall = true)]
pub struct CmdParser {
    #[command(subcommand)]
    pub command: Command,
}
