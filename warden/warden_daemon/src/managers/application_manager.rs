use crate::utils::repository::Repository;

use super::{
    application::{
        Application, ApplicationConfig, ApplicationData, ApplicationDisk, ApplicationError,
    },
    realm_client::RealmClient,
};

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct ApplicationManager {
    uuid: Uuid,
    config: Box<dyn Repository<Data = ApplicationConfig> + Send + Sync>,
    application_disk: Box<dyn ApplicationDisk + Send + Sync>,
    realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
}

impl ApplicationManager {
    pub async fn new(
        uuid: Uuid,
        config: Box<dyn Repository<Data = ApplicationConfig> + Send + Sync>,
        application_disk: Box<dyn ApplicationDisk + Send + Sync>,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Result<Self, ApplicationError> {
        application_disk.create_disk_with_partitions().await?;
        Ok(ApplicationManager {
            uuid,
            config,
            application_disk,
            realm_client_handler,
        })
    }
}

#[async_trait]
impl Application for ApplicationManager {
    async fn stop(&mut self) -> Result<(), ApplicationError> {
        self.realm_client_handler
            .lock()
            .await
            .stop_application(&self.uuid)
            .await
            .map_err(|err| ApplicationError::ApplicationStop(err.to_string()))?;
        Ok(())
    }
    async fn start(&mut self) -> Result<(), ApplicationError> {
        self.realm_client_handler
            .lock()
            .await
            .start_application(&self.uuid)
            .await
            .map_err(|err| ApplicationError::ApplicationStart(err.to_string()))?;
        Ok(())
    }

    async fn get_data(&self) -> Result<ApplicationData, ApplicationError> {
        let config = self.config.get();
        Ok(ApplicationData {
            id: self.uuid,
            name: config.name.clone(),
            version: config.version.clone(),
            image_registry: config.image_registry.clone(),
            image_part_uuid: self.application_disk.get_image_partition_uuid().await?,
            data_part_uuid: self.application_disk.get_data_partition_uuid().await?,
        })
    }

    async fn configure_disk(&mut self) -> Result<(), ApplicationError> {
        let config = self.config.get();
        self.application_disk
            .update_disk_with_partitions(config.data_storage_size_mb, config.image_storage_size_mb)
            .await
    }

    async fn update_config(&mut self, config: ApplicationConfig) -> Result<(), ApplicationError> {
        let own_config = self.config.get_mut();
        *own_config = config;
        self.config
            .save()
            .await
            .map_err(|err| ApplicationError::ConfigUpdate(err.to_string()))
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::utils::repository::RepositoryError;
    use crate::utils::test_utilities::{
        MockApplicationDisk, MockApplicationRepository, MockRealmClient,
    };
    use crate::{
        managers::{
            application::{Application, ApplicationError},
            realm_client::RealmClientError,
        },
        utils::test_utilities::create_example_app_config,
    };

    use super::ApplicationManager;

    #[tokio::test]
    async fn new() {
        let config = MockApplicationRepository::new();
        let mut disk = MockApplicationDisk::new();
        disk.expect_create_disk_with_partitions()
            .returning(|| Ok(()));
        let realm_client = MockRealmClient::new();
        let application_manager = ApplicationManager::new(
            Uuid::new_v4(),
            Box::new(config),
            Box::new(disk),
            Arc::new(tokio::sync::Mutex::new(Box::new(realm_client))),
        )
        .await;
        assert!(application_manager.is_ok());
    }

    #[tokio::test]
    async fn new_disk_error() {
        let config = MockApplicationRepository::new();
        let mut disk = MockApplicationDisk::new();
        disk.expect_create_disk_with_partitions()
            .returning(|| Err(ApplicationError::DiskOpertaion(String::new())));
        let realm_client = MockRealmClient::new();
        let application_manager = ApplicationManager::new(
            Uuid::new_v4(),
            Box::new(config),
            Box::new(disk),
            Arc::new(tokio::sync::Mutex::new(Box::new(realm_client))),
        )
        .await;
        assert!(matches!(
            application_manager,
            Err(ApplicationError::DiskOpertaion(_))
        ));
    }

    #[tokio::test]
    async fn stop() {
        let mut application_manager = create_application_manager(None, None, None).await;
        assert!(application_manager.stop().await.is_ok());
    }

    #[tokio::test]
    async fn stop_fail() {
        let mut realm_client = MockRealmClient::new();
        realm_client
            .expect_stop_application()
            .returning(|_| Err(RealmClientError::RealmConnectionFail(String::from(""))));
        let mut application_manager =
            create_application_manager(Some(realm_client), None, None).await;
        assert_eq!(
            application_manager.stop().await,
            Err(ApplicationError::ApplicationStop(
                RealmClientError::RealmConnectionFail(String::from("")).to_string()
            ))
        );
    }

    #[tokio::test]
    async fn start() {
        let mut application_manager = create_application_manager(None, None, None).await;
        assert!(application_manager.start().await.is_ok());
    }

    #[tokio::test]
    async fn start_fail() {
        let mut realm_client = MockRealmClient::new();
        realm_client
            .expect_start_application()
            .returning(|_| Err(RealmClientError::RealmConnectionFail(String::from(""))));
        let mut application_manager =
            create_application_manager(Some(realm_client), None, None).await;
        assert_eq!(
            application_manager.start().await,
            Err(ApplicationError::ApplicationStart(
                RealmClientError::RealmConnectionFail(String::from("")).to_string()
            ))
        );
    }

    #[tokio::test]
    async fn update() {
        let mut repository = MockApplicationRepository::new();
        repository
            .expect_get_mut()
            .return_var(create_example_app_config());
        repository.expect_save().returning(|| Ok(()));
        let mut application_manager =
            create_application_manager(None, Some(repository), None).await;
        assert!(application_manager
            .update_config(create_example_app_config())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn update_fail() {
        let mut repository = MockApplicationRepository::new();
        repository
            .expect_get_mut()
            .return_var(create_example_app_config());
        repository
            .expect_save()
            .returning(|| Err(RepositoryError::SaveFail(String::new())));
        let mut application_manager =
            create_application_manager(None, Some(repository), None).await;
        assert!(matches!(
            application_manager
                .update_config(create_example_app_config())
                .await,
            Err(ApplicationError::ConfigUpdate(_))
        ));
    }

    #[tokio::test]
    async fn get_data() {
        let application_manager = create_application_manager(None, None, None).await;
        assert!(application_manager.get_data().await.is_ok());
    }

    #[tokio::test]
    async fn prepare_for_next_run() {
        let mut application_manager = create_application_manager(None, None, None).await;
        assert!(application_manager.configure_disk().await.is_ok());
    }

    #[tokio::test]
    async fn prepare_for_next_run_error() {
        let mut disk = MockApplicationDisk::new();
        disk.expect_update_disk_with_partitions()
            .returning(|_, _| Err(ApplicationError::DiskOpertaion(String::new())));
        let mut application_manager = create_application_manager(None, None, Some(disk)).await;
        assert!(matches!(
            application_manager.configure_disk().await,
            Err(ApplicationError::DiskOpertaion(_))
        ));
    }

    #[tokio::test]
    async fn get_data_image_partition_missing() {
        let mut disk = MockApplicationDisk::new();
        disk.expect_get_data_partition_uuid()
            .returning(|| Ok(Uuid::new_v4()));
        disk.expect_get_image_partition_uuid()
            .returning(|| Err(ApplicationError::DiskOpertaion(String::new())));
        let application_manager = create_application_manager(None, None, Some(disk)).await;
        assert!(matches!(
            application_manager.get_data().await,
            Err(ApplicationError::DiskOpertaion(_))
        ));
    }

    #[tokio::test]
    async fn get_data_partition_missing() {
        let mut disk = MockApplicationDisk::new();
        disk.expect_get_image_partition_uuid()
            .returning(|| Ok(Uuid::new_v4()));
        disk.expect_get_data_partition_uuid()
            .returning(|| Err(ApplicationError::DiskOpertaion(String::new())));
        let application_manager = create_application_manager(None, None, Some(disk)).await;
        assert!(matches!(
            application_manager.get_data().await,
            Err(ApplicationError::DiskOpertaion(_))
        ));
    }

    async fn create_application_manager(
        realm_client: Option<MockRealmClient>,
        repository: Option<MockApplicationRepository>,
        application_disk: Option<MockApplicationDisk>,
    ) -> ApplicationManager {
        let realm_client = realm_client.unwrap_or({
            let mut realm_client = MockRealmClient::new();
            realm_client
                .expect_start_application()
                .returning(|_| Ok(()));
            realm_client.expect_stop_application().returning(|_| Ok(()));
            realm_client
        });
        let repository_mock = repository.unwrap_or({
            let mut mock = MockApplicationRepository::new();
            mock.expect_get().return_const(create_example_app_config());
            mock.expect_get_mut()
                .return_var(create_example_app_config());
            mock.expect_save().returning(|| Ok(()));
            mock
        });
        let application_disk = application_disk.unwrap_or({
            let mut mock = MockApplicationDisk::new();
            mock.expect_create_disk_with_partitions()
                .returning(|| Ok(()));
            mock.expect_get_data_partition_uuid()
                .returning(|| Ok(Uuid::new_v4()));
            mock.expect_get_image_partition_uuid()
                .returning(|| Ok(Uuid::new_v4()));
            mock.expect_update_disk_with_partitions()
                .returning(|_, _| Ok(()));
            mock
        });
        ApplicationManager {
            uuid: Uuid::new_v4(),
            config: Box::new(repository_mock),
            application_disk: Box::new(application_disk),
            realm_client_handler: Arc::new(Mutex::new(Box::new(realm_client))),
        }
    }
}
