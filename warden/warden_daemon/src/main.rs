use anyhow::Error;
use clap::Parser;
use warden_daemon::{app::App, cli::Cli};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<(), Error> {
    env_logger::init();
    let cli = Cli::parse();

    let app = App::new(cli).await?;
    app.run().await?.await?
}
