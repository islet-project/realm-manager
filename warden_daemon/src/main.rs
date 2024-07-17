use command_handler::client_command_handler::ClientHandler;
use fabric::application_fabric::ApplicationFabric;
use fabric::realm_manager_fabric::RealmManagerFabric;
use log::info;
use managers::application::ApplicationCreator;
use managers::realm::RealmCreator;
use managers::warden::Warden;
use managers::warden_manager::WardenDaemon;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tokio_vsock::VMADDR_CID_HOST;
use socket::unix_socket_server::UnixSocketServer;
use socket::vsocket_server::{VSockServer, VSockServerConfig};

mod managers;
mod socket;
mod virtualization;
mod command_handler;
mod fabric;

#[tokio::main]
async fn main() {
    env_logger::init();

    info!("Starting application!");
    let cid = VMADDR_CID_HOST; // Can there be other host?
    let port = 12345;
    let cancel_token = Arc::new(CancellationToken::new());
    let vsock_server = Arc::new(Mutex::new(VSockServer::new(
        VSockServerConfig { cid, port },
        cancel_token.clone(),
    )));

    let application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>> = Arc::new(Box::new(ApplicationFabric::new()));
    let realm_fabric: Box<dyn RealmCreator + Send + Sync> = Box::new(RealmManagerFabric::new(String::from("qemu-system-x86_64"), vsock_server.clone(), application_fabric)); 
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
            PathBuf::from("/tmp/unix-socket"),
        )
        .await
    });
    sleep(tokio::time::Duration::from_secs(100)).await;
    cancel_token.cancel();
    let _ = vsock_thread.await.unwrap();
    let _ = usock_thread.await.unwrap();
}
