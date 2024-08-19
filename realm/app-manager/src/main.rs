use clap::Parser;
use log::info;
use thiserror::Error;

use cli::Args;
use config::Config;
use manager::Manager;
use error::Result;

mod app;
mod cli;
mod error;
mod config;
mod dm;
mod key;
mod launcher;
mod manager;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    simple_logger::init_with_level(args.log_level)
        .expect("Cannot initialize logger.");

    info!("Reading config file: {:?}", args.config);
    let config = Config::read_from_file(args.config).await?;
    let mut manager = Manager::new(config).await?;

    info!("Provishioning...");
    manager.setup().await?;

    info!("Applications started entering event loop");
    manager.handle_events().await?;

    Ok(())
}
