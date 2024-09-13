use crate::virtualization::dnsmasq_server_handler::DnsmasqServerHandler;
use crate::virtualization::nat_manager::NetworkManagerHandler;
use crate::virtualization::network::NetworkConfig;
use crate::virtualization::network::NetworkManager;
use crate::virtualization::vm_runner::lkvm::LkvmRunner;
use crate::virtualization::vm_runner::qemu::QemuRunner;
use crate::virtualization::vm_runner::VmRunner;

use super::cli::Cli;
use super::client_handler::client_command_handler::ClientHandler;
use super::fabric::realm_fabric::RealmManagerFabric;
use super::fabric::warden_fabric::WardenFabric;
use super::managers::warden::Warden;
use super::socket::unix_socket_server::{UnixSocketServer, UnixSocketServerError};
use super::socket::vsocket_server::{VSockServer, VSockServerConfig, VSockServerError};
use anyhow::Error;
use log::{debug, error, info};
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct Daemon {
    vsock_server: Arc<Mutex<VSockServer>>,
    usock_server: UnixSocketServer,
    warden: Box<dyn Warden + Send + Sync>,
    network_manager: Arc<Mutex<NetworkManagerHandler<DnsmasqServerHandler>>>,
    cancellation_token: Arc<CancellationToken>,
}

impl Daemon {
    pub async fn new(cli: Cli) -> anyhow::Result<Self, Error> {
        let vsock_server = Arc::new(Mutex::new(VSockServer::new(VSockServerConfig {
            cid: cli.cid,
            port: cli.port,
        })));
        let mut udhcp_server =
            DnsmasqServerHandler::new(&cli.dhcp_exec_path, cli.dhcp_total_clients)?;
        udhcp_server.add_dns_args(cli.dns_records);
        let network_manager = Arc::new(Mutex::new(
            NetworkManagerHandler::create_nat(
                NetworkConfig {
                    net_if_name: cli.bridge_name,
                    net_if_ip: cli.network_address,
                },
                udhcp_server,
            )
            .await?,
        ));
        let realm_fabric = Box::new(RealmManagerFabric::new(
            Box::new(move |path, realm_id, config| {
                Ok(if cli.lkvm_runner {
                    let mut runner = LkvmRunner::new(cli.virtualizer_path.clone(), config);
                    if cli.cca_enable {
                        runner.configure_cca_settings();
                    }
                    Box::new(VmRunner::new(runner, realm_id, path))
                } else {
                    Box::new(VmRunner::new(
                        QemuRunner::new(cli.virtualizer_path.clone(), config),
                        realm_id,
                        path,
                    ))
                })
            }),
            vsock_server.clone(),
            cli.warden_workdir_path.clone(),
            network_manager.clone(),
            Duration::from_secs(cli.realm_connection_wait_time_secs),
            Duration::from_secs(cli.realm_response_wait_time_secs),
        ));
        let warden = WardenFabric::new(cli.warden_workdir_path)
            .await?
            .create_warden(realm_fabric)
            .await?;
        let usock_server = UnixSocketServer::new(&cli.unix_sock_path)?;
        Ok(Self {
            vsock_server,
            warden,
            usock_server,
            network_manager,
            cancellation_token: Arc::new(CancellationToken::new()),
        })
    }

    pub async fn run(self) -> anyhow::Result<JoinHandle<Result<(), Error>>, Error> {
        info!("Starting application.");
        let mut vsock_thread = Self::spawn_vsock_server_thread(
            self.vsock_server.clone(),
            self.cancellation_token.clone(),
        );
        let mut usock_thread = Self::spawn_unix_socket_server_thread(
            self.usock_server,
            self.warden,
            self.cancellation_token.clone(),
        );
        let mut sigint = signal(SignalKind::interrupt())?;
        let mut sigterm = signal(SignalKind::terminate())?;

        Ok(spawn(async move {
            select! {
                _ = sigint.recv() => {
                    info!("SIGINT received shutting down");
                }

                _ = sigterm.recv() => {
                    info!("SIGTERM recevied shutting down");
                }

                v = &mut vsock_thread => {
                    error!("Error while listening on vsocket: {:?}", v);
                }

                v = &mut usock_thread => {
                    error!("Error while listening on unixsocket: {:?}", v);
                }
            }
            info!("Shutting down application.");
            self.cancellation_token.cancel();

            if let Err(err) = self.network_manager.lock().await.shutdown_nat().await {
                error!("Failed to shutdown network manager: {err}");
            }

            if !vsock_thread.is_finished() {
                debug!("VSockServer result: {:#?}", vsock_thread.await);
            }

            if !usock_thread.is_finished() {
                debug!("UnixSocketServer result: {:#?}", usock_thread.await);
            }

            info!("Application succesfully shutdown.");
            Ok(())
        }))
    }

    fn spawn_vsock_server_thread(
        server: Arc<Mutex<VSockServer>>,
        token: Arc<CancellationToken>,
    ) -> JoinHandle<Result<(), VSockServerError>> {
        tokio::spawn(async move { VSockServer::listen(server, token).await })
    }

    fn spawn_unix_socket_server_thread(
        usock_server: UnixSocketServer,
        warden: Box<dyn Warden + Send + Sync>,
        token: Arc<CancellationToken>,
    ) -> JoinHandle<Result<(), UnixSocketServerError>> {
        tokio::spawn(async move { usock_server.listen::<ClientHandler>(warden, token).await })
    }
}
