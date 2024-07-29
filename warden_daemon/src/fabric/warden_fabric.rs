use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{
    managers::{
        warden::{RealmCreator, Warden},
        warden_manager::WardenDaemon,
    },
    storage::read_subfolders_uuids,
};
use tokio::sync::Mutex;

pub struct WardenFabric;

impl WardenFabric {
    pub async fn create_warden(
        realm_creator: Box<dyn RealmCreator + Send + Sync>,
        warden_workdir_path: PathBuf,
    ) -> Result<Box<dyn Warden + Send + Sync>, anyhow::Error> {
        let mut realms = HashMap::new();

        for uuid in read_subfolders_uuids(&warden_workdir_path).await? {
            realms.insert(
                uuid,
                Arc::new(Mutex::new(realm_creator.load_realm(&uuid).await?)),
            );
        }

        Ok(Box::new(WardenDaemon::new(realms, realm_creator)))
    }
}
