use clap::Parser;
use cmd_handler::CommandHanlder;
use cmd_parser::CmdParser;
use log::{error, info};
use std::path::PathBuf;
use utils::parse_users_input;
use client_lib::WardenConnection;

mod cmd_handler;
mod cmd_parser;
mod commands;
mod utils;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(short, long)]
    unix_socket_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    info!("Starting Warden cmd client! Press Ctrl^C to exit.");
    let cli_args = Args::parse();
    let warden_connection = WardenConnection::connect(cli_args.unix_socket_path).await?;
    let mut handler = CommandHanlder::new(warden_connection);
    loop {
        info!("Insert new command:");
        match parse_users_input() {
            Ok(cmd) => handle_cmd(cmd, &mut handler).await,
            Err(err) => error!("{}", err.to_string())
        }
    }
}

async fn handle_cmd(cmd: CmdParser, handler:& mut CommandHanlder) {
    match handler.handle_command(cmd.command).await {
        Err(err) => error!("Error occured why handling command: {:#?}!", err),
        Ok(_) => info!("Command handled successfully."),
    }
}