use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{
    managers::{
        warden::{RealmCreator, Warden},
        warden_manager::WardenDaemon,
    },
    storage::read_subfolders_uuids,
};
use tokio::sync::Mutex;
pub struct WardenFabric {
    warden_workdir_path: PathBuf,
}

impl WardenFabric {
    pub async fn new(warden_workdir_path: PathBuf) -> Result<Self, anyhow::Error> {
        tokio::fs::create_dir_all(&warden_workdir_path).await?;
        Ok(Self {
            warden_workdir_path,
        })
    }
}

impl WardenFabric {
    pub async fn create_warden(
        &self,
        realm_creator: Box<dyn RealmCreator + Send + Sync>,
    ) -> Result<Box<dyn Warden + Send + Sync>, anyhow::Error> {
        let mut realms = HashMap::new();

        for uuid in read_subfolders_uuids(&self.warden_workdir_path).await? {
            realms.insert(
                uuid,
                Arc::new(Mutex::new(realm_creator.load_realm(&uuid).await?)),
            );
        }

        Ok(Box::new(WardenDaemon::new(realms, realm_creator)))
    }
}
