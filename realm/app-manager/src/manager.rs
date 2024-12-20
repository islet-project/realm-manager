use std::{collections::HashMap, os::unix::process::ExitStatusExt};

use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::task::{JoinError, JoinSet};
use tokio_vsock::{VsockAddr, VsockStream, VMADDR_CID_HOST};
use utils::serde::json_framed::{JsonFramed, JsonFramedError};
use uuid::Uuid;
use warden_realm::{ApplicationInfo, ProtocolError, Request, Response};

use crate::app::Application;
use crate::config::{Config, KeySealingType, LauncherType, OciLauncherConfig, TokenResolver};
use crate::consts::RSI_KO;
use crate::error::Error;
use crate::key::dummy::DummyKeySealingFactory;
use crate::key::hkdf::HkdfSealingFactory;
use crate::key::KeySealing;
use crate::key::KeySealingFactory;
use crate::launcher::handler::ApplicationHandlerError;
use crate::launcher::oci::OciLauncher;
use crate::launcher::ApplicationHandler;
use crate::launcher::{dummy::DummyLauncher, Launcher};
use crate::util::crypto::EcdsaKey;
use crate::util::fs::read_to_vec;
use crate::util::net::read_if_addrs;
use crate::util::os::{insmod, reboot, SystemPowerAction};

use super::Result;
pub type ProtocolResult<T> = std::result::Result<T, ProtocolError>;

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("Failed to join the provisioning thread")]
    ProvisionJoinError(#[source] JoinError),

    #[error("Vsock connection error")]
    VsockConnectionError(#[source] std::io::Error),

    #[error("Framed json error")]
    FramedJsonError(#[from] JsonFramedError),

    #[error("Rem extension error")]
    RemExtensionError(#[source] nix::errno::Errno),

    #[error("Application yield no measurements")]
    AppHasNoMeasurements(),

    #[error("Application {0} not found")]
    AppNotFound(Uuid),

    #[error("Appplications already provisioned")]
    AlreadyProvisioned(),

    #[error("App-manager is not configured for remote attestation")]
    RaTlsIsNotSelected(),

    #[error("Failed to fetch attestation token from RSI")]
    RsiTokenFetch(#[source] rust_rsi::NixError),

    #[error("Invalid challenge size expected 64 bytes")]
    InvalidChallengeSize(),
}

pub struct Manager {
    config: Config,
    apps: HashMap<Uuid, (Application, Box<dyn ApplicationHandler + Send + Sync>)>,
    conn: JsonFramed<VsockStream, Request, Response>,
    sealing_factory: Box<dyn KeySealingFactory + Send + Sync>,
}

impl Manager {
    pub async fn new(config: Config) -> Result<Self> {
        if config.requires_rsi() {
            info!("Loading rsi kernel module");
            insmod(RSI_KO, "").await?;
        }

        let vsock = VsockStream::connect(VsockAddr::new(VMADDR_CID_HOST, config.vsock_port))
            .await
            .map_err(ManagerError::VsockConnectionError)?;
        info!("Connected to warden daemon");

        info!("Initializing key sealing");
        let sealing_factory: Box<dyn KeySealingFactory + Send + Sync> = match &config.keysealing {
            KeySealingType::Dummy => Box::new(DummyKeySealingFactory::new(vec![0x11, 0x22, 0x33])),
            KeySealingType::HkdfSha256(ikm_source) => {
                Box::new(HkdfSealingFactory::new(ikm_source)?)
            }
        };

        Ok(Self {
            config,
            apps: HashMap::new(),
            conn: JsonFramed::new(vsock),
            sealing_factory,
        })
    }

    async fn make_launcher(&self) -> Result<Box<dyn Launcher + Send + Sync>> {
        match &self.config.launcher {
            LauncherType::Dummy => Ok(Box::new(DummyLauncher::new())),
            LauncherType::Oci(cfg) => {
                let ca = read_to_vec(&self.config.ca_pub).await?;
                let ca_key = EcdsaKey::import(ca)?;

                Ok(Box::new(OciLauncher::from_oci_config(cfg.clone(), ca_key)))
            }
        }
    }

    fn make_keyseal(&self) -> Result<Box<dyn KeySealing + Send + Sync>> {
        Ok(self.sealing_factory.create())
    }

    async fn recv_msg(&mut self) -> Result<Request> {
        let msg = self
            .conn
            .recv()
            .await
            .map_err(ManagerError::FramedJsonError)?;

        Ok(msg)
    }

    async fn send_msg(&mut self, resp: Response) -> Result<()> {
        self.conn
            .send(resp)
            .await
            .map_err(ManagerError::FramedJsonError)?;

        Ok(())
    }

    async fn provision(&mut self, apps_info: &[ApplicationInfo]) -> Result<()> {
        if !self.apps.is_empty() {
            return Err(ManagerError::AlreadyProvisioned().into());
        }

        info!("Starting installation");

        let mut set =
            JoinSet::<Result<(Application, Box<dyn ApplicationHandler + Send + Sync>)>>::new();

        for app_info in apps_info.iter() {
            let app_dir = self.config.workdir.join(app_info.id.to_string());
            let launcher = self.make_launcher().await?;
            let keyseal = self.make_keyseal()?;
            let params = self.config.crypto.clone();
            let info = app_info.clone();

            set.spawn(async move {
                let mut app = Application::new(info, app_dir)?;
                let handler = app.setup(params, launcher, keyseal).await.map_err(|e| {
                    error!("Application installation error: {:?}", e);
                    e
                })?;

                Ok((app, handler))
            });
        }

        while let Some(result) = set.join_next().await {
            let (app, handler) = result.map_err(ManagerError::ProvisionJoinError)??;
            let id = *app.id();
            self.apps.insert(id, (app, handler));
            info!("Finished installing {}", id);
        }

        for info in apps_info.iter() {
            let (app, _) = self
                .apps
                .get(&info.id)
                .ok_or(ManagerError::AppNotFound(info.id))?;
            info!("Measuring app {}", info.id);
            self.extend_rem(app.measurements())?;
        }

        if self.config.autostartall {
            for (id, (_, handler)) in self.apps.iter_mut() {
                info!("Starting app {}", id);
                handler.start().await?;
            }
        }

        info!("Provisioning finished");

        Ok(())
    }

    async fn read_attestation_token(&mut self, challenge: &[u8]) -> Result<Vec<u8>> {
        info!("Reading attestation token");

        match &self.config.launcher {
            LauncherType::Oci(crate::config::OciLauncherConfig::RaTLS {
                token_resolver: TokenResolver::Rsi,
                ..
            }) => {
                info!("Fetching token from RSI");
                let chall: &[u8; 64] = challenge
                    .try_into()
                    .map_err(|_| ManagerError::InvalidChallengeSize())?;

                Ok(
                    tokio::task::block_in_place(|| rust_rsi::attestation_token(chall))
                        .map_err(ManagerError::RsiTokenFetch)?,
                )
            }

            LauncherType::Oci(OciLauncherConfig::RaTLS {
                token_resolver: TokenResolver::File(path),
                ..
            }) => {
                info!("Using attestation token from file");

                Ok(read_to_vec(path).await?)
            }

            _ => Err(ManagerError::RaTlsIsNotSelected().into()),
        }
    }

    fn extend_rem(&self, data: &[u8]) -> Result<()> {
        if let Some(rem) = self.config.extend.as_ref() {
            if data.is_empty() {
                return Err(ManagerError::AppHasNoMeasurements().into());
            }

            for chunk in data.chunks(rust_rsi::MAX_MEASUR_LEN as usize) {
                debug!("Extending {:?} with {:?}", rem, chunk);
                rust_rsi::measurement_extend(*rem as u32, chunk)
                    .map_err(ManagerError::RemExtensionError)?;
            }
        }

        Ok(())
    }

    fn get_handler(
        &mut self,
        id: &Uuid,
    ) -> ProtocolResult<&mut (dyn ApplicationHandler + Send + Sync)> {
        Ok(self
            .apps
            .get_mut(id)
            .ok_or(ProtocolError::ApplicationNotFound())?
            .1
            .as_mut())
    }

    async fn shutdown_all_apps(&mut self) {
        info!("Shutting down all applications");

        for (id, (app, handler)) in self.apps.iter_mut() {
            if let Err(e) = handler.stop().await {
                warn!("Failed to stop app {:?}, error: {:?}", id, e);
            }

            if let Err(e) = app.cleanup().await {
                warn!("Failed to cleanup app {:?}, error: {:?}", id, e);
            }
        }
    }

    async fn perform_reboot(&mut self, action: SystemPowerAction) -> ProtocolResult<Response> {
        self.shutdown_all_apps().await;
        let e = reboot(action);
        Err(ProtocolError::SystemPowerActionFailed(format!("{:?}", e)))
    }

    async fn handle_request(&mut self, request: Request) -> ProtocolResult<Response> {
        match request {
            Request::ProvisionInfo(apps_info) => {
                info!("Received provisioning request");

                match self.provision(apps_info.as_slice()).await {
                    Ok(()) => Ok(Response::Success()),
                    Err(e) => Err(ProtocolError::ProvisioningError(format!("{:?}", e))),
                }
            }

            Request::GetAttestationToken(challenge) => {
                info!("Fetching attestation token");

                match self.read_attestation_token(challenge.as_slice()).await {
                    Ok(token) => Ok(Response::AttestationToken(token)),
                    Err(e) => Err(ProtocolError::AttestationTokenReadingError(format!(
                        "{:?}",
                        e
                    ))),
                }
            }

            Request::GetIfAddrs() => {
                info!("Reading ip addresses of network interfaces");

                match read_if_addrs() {
                    Ok(net_addrs) => Ok(Response::IfAddrs(net_addrs)),
                    Err(e) => Err(ProtocolError::GetIfAddrsError(format!("{:?}", e))),
                }
            }

            Request::StartApp(id) => {
                info!("Starting application: {:?}", id);
                let handler = self.get_handler(&id)?;

                match handler.start().await {
                    Ok(()) => Ok(Response::Success()),
                    Err(e) => Err(ProtocolError::ApplicationLaunchFailed(format!("{:?}", e))),
                }
            }

            Request::StopApp(id) => {
                info!("Stopping application: {:?}", id);
                let handler = self.get_handler(&id)?;

                match handler.stop().await {
                    Ok(()) => Ok(Response::Success()),
                    Err(e) => Err(ProtocolError::ApplicationStopFailed(format!("{:?}", e))),
                }
            }

            Request::KillApp(id) => {
                info!("Killing application: {:?}", id);
                let handler = self.get_handler(&id)?;

                match handler.kill().await {
                    Ok(()) => Ok(Response::Success()),
                    Err(e) => Err(ProtocolError::ApplicationKillFailed(format!("{:?}", e))),
                }
            }

            Request::CheckStatus(id) => {
                info!("Checking if application is running: {:?}", id);
                let handler = self.get_handler(&id)?;

                match handler.try_wait().await {
                    Ok(Some(status)) => Ok(Response::ApplicationExited(status.into_raw())),
                    Ok(None) => Ok(Response::ApplicationIsRunning()),
                    Err(Error::Handler(ApplicationHandlerError::AppNotRunning())) => {
                        Ok(Response::ApplicationNotStarted())
                    }
                    Err(e) => Err(ProtocolError::ApplicationCheckStatusFailed(format!(
                        "{:?}",
                        e
                    ))),
                }
            }

            Request::Shutdown() => {
                info!("Performing system shutdown");
                self.perform_reboot(SystemPowerAction::Shutdown).await
            }

            Request::Reboot() => {
                info!("Performing system reboot");
                self.perform_reboot(SystemPowerAction::Reboot).await
            }
        }
    }

    async fn handle_received_request(&mut self, request: Request) -> Response {
        debug!("Received request: {:?}", request);

        match self.handle_request(request).await {
            Ok(response) => response,
            Err(e) => Response::Error(e),
        }
    }

    pub async fn handle_events(&mut self) -> Result<()> {
        loop {
            let response = match self.recv_msg().await {
                Ok(r) => self.handle_received_request(r).await,

                Err(e) => Response::Error(ProtocolError::InvalidRequest(format!("{:?}", e))),
            };

            debug!("Sending response: {:?}", response);
            if let Err(e) = self.send_msg(response).await {
                error!("Failed to send data back to host ({})", e);
                info!("Shutting down");
                let _ = self.perform_reboot(SystemPowerAction::Shutdown).await;

                unreachable!()
            }
        }
    }
}
