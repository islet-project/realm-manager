use anyhow::Error;
use clap::Parser;
use warden_daemon::{cli::Cli, daemon::DaemonBuilder};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<(), Error> {
    env_logger::init();
    let cli = Cli::parse();

    let daemon = DaemonBuilder::build(cli).await?;
    let app_thread_handle = daemon.run().await?;
    app_thread_handle.await?
}
