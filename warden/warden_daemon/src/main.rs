use anyhow::Error;
use clap::Parser;
use warden_daemon::{cli::Cli, daemon::Daemon};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<(), Error> {
    env_logger::init();
    let cli = Cli::parse();

    let app = Daemon::new(cli).await?;
    let app_thread_handle = app.run().await?;
    app_thread_handle.await?
}
