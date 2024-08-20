use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to config file
    #[arg(short = 'c', long, default_value = "/etc/app-manager/config.yml")]
    pub config: PathBuf,

    /// Log level
    #[arg(short = 'l', long, default_value_t = log::Level::Info)]
    pub log_level: log::Level,
}
