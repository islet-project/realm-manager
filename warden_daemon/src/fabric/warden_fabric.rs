use std::{collections::HashMap, io, path::PathBuf, str::FromStr, sync::Arc};

use tokio::sync::Mutex;
use utils::file_system::workspace_manager::WorkspaceManager;
use uuid::Uuid;

use crate::managers::{
    warden::{RealmCreator, Warden},
    warden_manager::WardenDaemon,
};

pub struct WardenFabric;

impl WardenFabric {
    pub async fn create_warden(
        realm_creator: Box<dyn RealmCreator + Send + Sync>,
        warden_workdir_path: PathBuf,
    ) -> Result<Box<dyn Warden + Send + Sync>, anyhow::Error> {
        let workspace_manager = WorkspaceManager::new(warden_workdir_path).await?;
        let mut realms = HashMap::new();

        for realm_dir in workspace_manager.read_subdirectories().await?.into_iter() {
            let realm_uuid = {
                let realm_uuid = realm_dir.iter().last().ok_or(io::Error::other(
                    "Unable to collect realm's Uuid from child dir.",
                ))?;
                Uuid::from_str(
                    realm_uuid
                        .to_str()
                        .ok_or(io::Error::other("Failed to map OS string."))?,
                )
                .map_err(|err| io::Error::other(err.to_string()))?
            };
            let realm = realm_creator.load_realm(&realm_uuid).await?;
            realms.insert(realm_uuid, Arc::new(Mutex::new(realm)));
        }

        Ok(Box::new(WardenDaemon::new(realms, realm_creator)))
    }
}
