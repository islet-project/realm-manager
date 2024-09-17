use anyhow::Error;
use clap::Parser;
use log::{debug, error};
use warden_daemon::{cli::Cli, daemon::DaemonBuilder};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<(), Error> {
    env_logger::init();
    let cli = Cli::parse();

    let mut daemon_builder = DaemonBuilder::default();
    match daemon_builder.build_daemon(cli).await {
        Ok(app) => {
            let app_thread_handle = app.run().await?;
            app_thread_handle.await?
        }
        Err(err) => {
            error!("{}", err);
            debug!("Cleaning up after error.");
            daemon_builder.cleanup().await
        }
    }
}
