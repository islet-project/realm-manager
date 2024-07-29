use crate::client_handler::realm_client_handler::RealmClientHandler;
use crate::managers::application::ApplicationCreator;
use crate::managers::realm::{Realm, RealmConfigRepository};
use crate::managers::realm_configuration::RealmConfig;
use crate::managers::realm_manager::RealmManager;
use crate::managers::warden::{RealmCreator, WardenError};
use crate::socket::vsocket_server::VSockServer;
use crate::storage::RealmConfigYamlRepository;
use crate::virtualization::qemu_runner::QemuRunner;

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use utils::file_system::fs_repository::FileRepository;
use uuid::Uuid;

pub struct RealmManagerFabric {
    qemu_path: PathBuf,
    vsock_server: Arc<Mutex<VSockServer>>,
    application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
    warden_workdir_path: PathBuf,
}

impl RealmManagerFabric {
    pub fn new(
        qemu_path: PathBuf,
        vsock_server: Arc<Mutex<VSockServer>>,
        application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
        warden_workdir_path: PathBuf,
    ) -> Self {
        RealmManagerFabric {
            qemu_path,
            vsock_server,
            application_fabric,
            warden_workdir_path,
        }
    }

    fn create_realm_config_path(mut warden_workdir_path: PathBuf, realm_id: &Uuid) -> PathBuf {
        const CONFIG_FILE_NAME: &str = "config.yaml";
        warden_workdir_path.push(realm_id.to_string());
        warden_workdir_path.push(CONFIG_FILE_NAME);
        warden_workdir_path
    }

    async fn create_realm_config_repository(
        config: RealmConfig,
        path: &Path,
    ) -> Result<RealmConfigYamlRepository, WardenError> {
        let mut realm_config_repository = RealmConfigYamlRepository::new(config, path)
            .await
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        realm_config_repository
            .save_realm_config()
            .await
            .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?;
        Ok(realm_config_repository)
    }

    async fn load_realm_config(
        config_path: &Path,
    ) -> Result<RealmConfigYamlRepository, WardenError> {
        Ok(RealmConfigYamlRepository::from(
            FileRepository::<RealmConfig>::from_file_path(config_path)
                .await
                .map_err(|err| WardenError::RealmCreationFail(err.to_string()))?,
        ))
    }
}

#[async_trait]
impl RealmCreator for RealmManagerFabric {
    async fn create_realm(
        &self,
        realm_id: Uuid,
        config: RealmConfig,
    ) -> Result<Box<dyn Realm + Send + Sync>, WardenError> {
        let path = RealmManagerFabric::create_realm_config_path(
            self.warden_workdir_path.clone(),
            &realm_id,
        );
        Ok(Box::new(RealmManager::new(
            Box::new(RealmManagerFabric::create_realm_config_repository(config, &path).await?),
            Box::new(QemuRunner::new(self.qemu_path.clone())),
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
            )))),
            self.application_fabric.clone(),
        )))
    }

    async fn load_realm(
        &self,
        realm_id: &Uuid,
    ) -> Result<Box<dyn Realm + Send + Sync>, WardenError> {
        let realm_folder_path = RealmManagerFabric::create_realm_config_path(
            self.warden_workdir_path.clone(),
            realm_id,
        );
        Ok(Box::new(RealmManager::new(
            Box::new(RealmManagerFabric::load_realm_config(&realm_folder_path).await?),
            Box::new(QemuRunner::new(self.qemu_path.clone())),
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
            )))),
            self.application_fabric.clone(),
        )))
    }
}

#[cfg(test)]
mod test {
    use std::{fs::remove_dir, path::PathBuf, str::FromStr};
    use uuid::Uuid;

    use crate::{
        managers::realm::RealmConfigRepository, test_utilities::create_example_realm_config,
    };

    use super::RealmManagerFabric;

    #[test]
    fn create_realm_config_path() {
        let uuid = Uuid::new_v4();
        let path = RealmManagerFabric::create_realm_config_path(PathBuf::new(), &uuid);
        assert_eq!(
            path,
            PathBuf::from_str(&format!("{}/config.yaml", uuid.to_string())).unwrap()
        );
    }

    #[tokio::test]
    async fn create_realm_config_repository() {
        const MACHINE: &str = "TEST_MACHINE";
        let uuid = Uuid::new_v4();
        let file_guard = FileGuard {
            path: PathBuf::from_str(&format!("/tmp/{}", uuid.to_string())).unwrap(),
        };

        let mut realm_config = create_example_realm_config();
        realm_config.machine = String::from(MACHINE);
        let repository =
            RealmManagerFabric::create_realm_config_repository(realm_config, &file_guard.path)
                .await;
        assert!(repository.is_ok());
        let mut loaded_repository = RealmManagerFabric::load_realm_config(&file_guard.path)
            .await
            .unwrap();
        assert_eq!(loaded_repository.get_realm_config().machine, MACHINE);
    }

    struct FileGuard {
        pub path: PathBuf,
    }

    impl Drop for FileGuard {
        fn drop(&mut self) {
            let _ = remove_dir(&self.path);
        }
    }
}
