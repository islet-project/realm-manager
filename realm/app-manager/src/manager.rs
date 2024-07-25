use std::{collections::HashMap, path::Path, sync::Arc};

use thiserror::Error;
use tokio::task::{JoinError, JoinSet};
use uuid::Uuid;
use log::info;

use crate::{app::{Application, ApplicationInfo}, config::{Config, KeySealingType, LauncherType}, dm::DeviceMapper, key::{dummy::DummyKeySealing, ring::KernelKeyring, KeySealing}, launcher::{dummy::DummyLauncher, Launcher}};

use super::Result;

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("Invalid launcher")]
    InvalidLauncher(),

    #[error("Failed to join the provisioning thread")]
    ProvisionJoinError(#[source] JoinError)
}

pub struct Manager {
    config: Config,
    apps: HashMap<Uuid, Application>
}

impl Manager {
    pub fn new(config: Config) -> Result<Self> {

        Ok(Self {
            config,
            apps: HashMap::new()
        })
    }

    fn make_launcher(&self) -> Result<Box<dyn Launcher + Send + Sync>> {
        match self.config.launcher {
             LauncherType::Dummy => Ok(Box::new(DummyLauncher::new())),
        }
    }

    fn make_keyseal(&self) -> Result<Box<dyn KeySealing + Send + Sync>> {
        match self.config.keysealing {
            KeySealingType::Dummy => Ok(Box::new(DummyKeySealing::new(vec![0x11, 0x22, 0x33])))
        }
    }

    pub async fn setup(&mut self, workdir: impl AsRef<Path>, apps_info: Vec<ApplicationInfo>) -> Result<()> {
        log::info!("Starting installation");

        let mut set = JoinSet::<Result<Application>>::new();

        for app_info in apps_info.into_iter() {
            let app_dir = workdir.as_ref().join(app_info.id.to_string());
            let launcher = self.make_launcher()?;
            let keyseal = self.make_keyseal()?;
            let params = self.config.crypto.clone();

            set.spawn(async move {
                let mut app = Application::new(app_info, app_dir)?;
                app.setup(params, launcher, keyseal).await?;

                Ok(app)
            });
        }

        while let Some(result) = set.join_next().await {
            let app = result
                .map_err(ManagerError::ProvisionJoinError)??;
            let id = app.id().clone();
            self.apps.insert(id, app);
            info!("Finished installing {}", id);
        }

        Ok(())

    }

    pub async fn handle_events(&mut self) -> Result<()> {
        todo!()
    }
}
