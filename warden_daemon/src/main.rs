use anyhow::Error;
use clap::Parser;
use command_handler::client_command_handler::ClientHandler;
use fabric::application_fabric::ApplicationFabric;
use fabric::realm_manager_fabric::RealmManagerFabric;
use log::{error, info};
use managers::application::ApplicationCreator;
use managers::realm::RealmCreator;
use managers::warden::Warden;
use managers::warden_manager::WardenDaemon;
use socket::unix_socket_server::{UnixSocketServer, UnixSocketServerError};
use socket::vsocket_server::{VSockServer, VSockServerConfig, VSockServerError};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_vsock::VMADDR_CID_HOST;

mod command_handler;
mod fabric;
mod managers;
mod socket;
mod virtualization;

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
    unix_sock_path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<(), Error> {
    env_logger::init();
    let cli = Cli::parse();

    info!("Starting application!");
    let cancel_token = Arc::new(CancellationToken::new());
    let vsock_server = Arc::new(Mutex::new(VSockServer::new(
        VSockServerConfig {
            cid: cli.cid,
            port: cli.port,
        },
        cancel_token.clone(),
    )));

    let application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>> =
        Arc::new(Box::new(ApplicationFabric::new()));
    let realm_fabric: Box<dyn RealmCreator + Send + Sync> = Box::new(RealmManagerFabric::new(
        cli.qemu_path,
        vsock_server.clone(),
        application_fabric,
    ));
    let warden: Box<dyn Warden + Send + Sync> = Box::new(WardenDaemon::new(realm_fabric));
    let mut vsock_thread = spawn_vsock_server_thread(vsock_server.clone(), cancel_token.clone());
    let mut usock_thread =
        spawn_unix_socket_server_thread(warden, cancel_token.clone(), cli.unix_sock_path);
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    select! {
        _ = sigint.recv() => {
            info!("SIGINT received shutting down");
            cancel_token.cancel();
        }

        _ = sigterm.recv() => {
            info!("SIGTERM recevied shuttding down");
            cancel_token.cancel();
        }

        v = &mut vsock_thread => {
            error!("Error while listening on unixsocket: {:?}", v);
            cancel_token.cancel();
        }

        v = &mut usock_thread => {
            error!("Error while listening on vsock: {:?}", v);
            cancel_token.cancel();
        }
    }

    info!("Shutting down application!");

    if !vsock_thread.is_finished() {
        vsock_thread.await??;
    }

    if !usock_thread.is_finished() {
        usock_thread.await??;
    }

    info!("Application succesfully shutdown!");
    Ok(())
}

fn spawn_vsock_server_thread(
    server: Arc<Mutex<VSockServer>>,
    token: Arc<CancellationToken>,
) -> JoinHandle<Result<(), VSockServerError>> {
    tokio::spawn(async move { VSockServer::listen(server, token).await })
}

fn spawn_unix_socket_server_thread(
    warden: Box<dyn Warden + Send + Sync>,
    token: Arc<CancellationToken>,
    socket_path: PathBuf,
) -> JoinHandle<Result<(), UnixSocketServerError>> {
    tokio::spawn(async move {
        UnixSocketServer::listen::<ClientHandler>(warden, token, socket_path).await
    })
}