use std::{collections::HashMap, os::unix::process::ExitStatusExt};

use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::task::{JoinError, JoinSet};
use tokio_vsock::{VsockAddr, VsockStream, VMADDR_CID_HOST};
use utils::serde::json_framed::{JsonFramed, JsonFramedError};
use uuid::Uuid;
use warden_realm::{ApplicationInfo, ProtocolError, Request, Response};

use crate::app::Application;
use crate::config::{Config, KeySealingType, LauncherType};
use crate::key::{dummy::DummyKeySealing, KeySealing};
use crate::launcher::handler::ApplicationHandlerError;
use crate::launcher::{dummy::DummyLauncher, Launcher};
use crate::launcher::{ApplicationHandler, LauncherError};
use crate::util::os::{reboot, SystemPowerAction};

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

    #[error("Invalid message received")]
    InvalidMessage(),
}

pub struct Manager {
    config: Config,
    apps: HashMap<Uuid, (Application, Box<dyn ApplicationHandler + Send + Sync>)>,
    conn: JsonFramed<VsockStream, Request, Response>,
}

impl Manager {
    pub async fn new(config: Config) -> Result<Self> {
        let vsock = VsockStream::connect(VsockAddr::new(VMADDR_CID_HOST, config.vsock_port))
            .await
            .map_err(ManagerError::VsockConnectionError)?;
        info!("Connected to warden daemon");

        Ok(Self {
            config,
            apps: HashMap::new(),
            conn: JsonFramed::new(vsock),
        })
    }

    fn make_launcher(&self) -> Result<Box<dyn Launcher + Send + Sync>> {
        match self.config.launcher {
            LauncherType::Dummy => Ok(Box::new(DummyLauncher::new())),
        }
    }

    fn make_keyseal(&self) -> Result<Box<dyn KeySealing + Send + Sync>> {
        match self.config.keysealing {
            KeySealingType::Dummy => Ok(Box::new(DummyKeySealing::new(vec![0x11, 0x22, 0x33]))),
        }
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

    async fn recv_provision_info(&mut self) -> Result<Vec<ApplicationInfo>> {
        let msg = self.recv_msg().await?;

        if let Request::ProvisionInfo(infos) = msg {
            Ok(infos)
        } else {
            error!("Provision info not received, got: {:?}", msg);

            Err(ManagerError::InvalidMessage().into())
        }
    }

    async fn report_provision_success(&mut self) -> Result<()> {
        self.conn
            .send(Response::Success())
            .await
            .map_err(ManagerError::FramedJsonError)?;

        Ok(())
    }

    pub async fn setup(&mut self) -> Result<()> {
        info!("Waiting for provision info");
        let apps_info = self.recv_provision_info().await?;
        debug!("Received provision info: {:?}", apps_info);

        info!("Starting installation");

        let mut set =
            JoinSet::<Result<(Application, Box<dyn ApplicationHandler + Send + Sync>)>>::new();
        let autostart = self.config.autostartall;

        for app_info in apps_info.into_iter() {
            let app_dir = self.config.workdir.join(app_info.id.to_string());
            let launcher = self.make_launcher()?;
            let keyseal = self.make_keyseal()?;
            let params = self.config.crypto.clone();

            set.spawn(async move {
                let mut app = Application::new(app_info, app_dir)?;
                let mut handler = app.setup(params, launcher, keyseal).await?;

                if autostart {
                    handler.start().await?;
                }

                Ok((app, handler))
            });
        }

        while let Some(result) = set.join_next().await {
            let (app, handler) = result.map_err(ManagerError::ProvisionJoinError)??;
            let id = *app.id();
            self.apps.insert(id, (app, handler));
            info!("Finished installing {}", id);
        }

        info!("Provisioning finished");
        self.report_provision_success().await?;

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
            Request::ProvisionInfo(_) => {
                error!("Application already provisioned!");
                Ok(Response::Error(
                    ProtocolError::ApplicationsAlreadyProvisioned(),
                ))
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
                    Err(LauncherError::HandlerError(ApplicationHandlerError::AppNotRunning())) => {
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
