use clap::Parser;
use log::info;

use cli::Args;
use config::Config;
use error::{Error, Result};
use manager::Manager;

mod app;
mod cli;
mod config;
mod consts;
mod dm;
mod error;
mod key;
mod launcher;
mod manager;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<(), Error> {
    let args = Args::parse();
    simple_logger::init_with_level(args.log_level).expect("Cannot initialize logger.");

    info!("Reading config file: {:?}", args.config);
    let config = Config::read_from_file(args.config).await?;
    let mut manager = Manager::new(config).await?;

    info!("Provishioning...");
    manager.setup().await?;

    info!("Applications started entering event loop");
    manager.handle_events().await?;

    Ok(())
}
