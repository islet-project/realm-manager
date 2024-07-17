use command_handler::client_command_handler::ClientHandler;
use fabric::application_fabric::ApplicationFabric;
use fabric::realm_manager_fabric::RealmManagerFabric;
use log::info;
use managers::application::ApplicationCreator;
use managers::realm::RealmCreator;
use managers::warden::Warden;
use managers::warden_manager::WardenDaemon;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tokio_vsock::VMADDR_CID_HOST;
use socket::unix_socket_server::UnixSocketServer;
use socket::vsocket_server::{VSockServer, VSockServerConfig};
use clap::Parser;

mod managers;
mod socket;
mod virtualization;
mod command_handler;
mod fabric;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(short, long, value_parser=clap::value_parser!(u32).range(2..), default_value_t = VMADDR_CID_HOST)]
    cid: u32,
    #[arg(short, long, value_parser=clap::value_parser!(u32).range(80..), default_value_t = 80)]
    port: u32,
    #[arg(short, long)]
    qemu_path: PathBuf,
    #[arg(short, long)]
    usock_path: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    info!("Starting application!");
    let cancel_token = Arc::new(CancellationToken::new());
    let vsock_server = Arc::new(Mutex::new(VSockServer::new(
        VSockServerConfig { cid: cli.cid, port: cli.port },
        cancel_token.clone(),
    )));

    let application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>> = Arc::new(Box::new(ApplicationFabric::new()));
    let realm_fabric: Box<dyn RealmCreator + Send + Sync> = Box::new(RealmManagerFabric::new(cli.qemu_path, vsock_server.clone(), application_fabric)); 
    let host_daemon: Arc<Mutex<Box<dyn Warden + Send + Sync>>> =
        Arc::new(Mutex::new(Box::new(WardenDaemon::new(
            realm_fabric
        ))));
    let server_clone = vsock_server.clone();
    let token_clone = cancel_token.clone();
    let vsock_thread =
        tokio::spawn(async move { VSockServer::listen(server_clone, token_clone.clone()).await });
    let token_clone = cancel_token.clone();
    let usock_thread = tokio::spawn(async move {
        UnixSocketServer::listen::<ClientHandler>(
            host_daemon.clone(),
            token_clone,
            cli.usock_path,
        )
        .await
    });
    sleep(tokio::time::Duration::from_secs(100)).await;
    cancel_token.cancel();
    let _ = vsock_thread.await.unwrap();
    let _ = usock_thread.await.unwrap();
}
