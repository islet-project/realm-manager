use crate::utils::repository::Repository;

use super::{
    application::{Application, ApplicationConfig, ApplicationData, ApplicationError},
    realm_client::RealmClient,
};

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct ApplicationManager {
    uuid: Uuid,
    config: Box<dyn Repository<Data = ApplicationConfig> + Send + Sync>,
    realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
}

impl ApplicationManager {
    pub fn new(
        uuid: Uuid,
        config: Box<dyn Repository<Data = ApplicationConfig> + Send + Sync>,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Self {
        ApplicationManager {
            uuid,
            config,
            realm_client_handler,
        }
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

    fn get_data(&self) -> ApplicationData {
        let config = self.config.get();
        ApplicationData {
            id: self.uuid,
            name: config.name.clone(),
            version: config.version.clone(),
            image_registry: config.image_registry.clone(),
            image_part_uuid: Uuid::new_v4(), // TODO: implement partition's creation
            data_part_uuid: Uuid::new_v4(),  // TODO: implement partition's creation
        }
    }

    async fn update(&mut self, config: ApplicationConfig) -> Result<(), ApplicationError> {
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

    use crate::utils::test_utilities::{MockApplicationRepository, MockRealmClient};
    use crate::{
        managers::{
            application::{Application, ApplicationError},
            realm_client::RealmClientError,
        },
        utils::test_utilities::create_example_app_config,
    };

    use super::ApplicationManager;

    #[tokio::test]
    async fn stop() {
        let mut application_manager = create_application_manager(None, None);
        assert!(application_manager.stop().await.is_ok());
    }

    #[tokio::test]
    async fn stop_fail() {
        let mut realm_client = MockRealmClient::new();
        realm_client
            .expect_stop_application()
            .returning(|_| Err(RealmClientError::RealmConnectionFail(String::from(""))));
        let mut application_manager = create_application_manager(Some(realm_client), None);
        assert_eq!(
            application_manager.stop().await,
            Err(ApplicationError::ApplicationStop(
                RealmClientError::RealmConnectionFail(String::from("")).to_string()
            ))
        );
    }

    #[tokio::test]
    async fn start() {
        let mut application_manager = create_application_manager(None, None);
        assert!(application_manager.start().await.is_ok());
    }

    #[tokio::test]
    async fn start_fail() {
        let mut realm_client = MockRealmClient::new();
        realm_client
            .expect_start_application()
            .returning(|_| Err(RealmClientError::RealmConnectionFail(String::from(""))));
        let mut application_manager = create_application_manager(Some(realm_client), None);
        assert_eq!(
            application_manager.start().await,
            Err(ApplicationError::ApplicationStart(
                RealmClientError::RealmConnectionFail(String::from("")).to_string()
            ))
        );
    }

    #[tokio::test]
    async fn update() {
        const APP_NEW_NAME: &str = "NEW_NAME";
        let mut repository = MockApplicationRepository::new();
        repository
            .expect_get_mut()
            .return_var(create_example_app_config());
        repository.expect_save().returning(|| Ok(()));
        let mut application_manager = create_application_manager(None, Some(repository));
        let mut app_config = create_example_app_config();
        app_config.name = APP_NEW_NAME.to_string();
        assert!(application_manager.update(app_config.clone()).await.is_ok());
    }

    fn create_application_manager(
        realm_client: Option<MockRealmClient>,
        repository: Option<MockApplicationRepository>,
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
            mock.expect_get_mut()
                .return_var(create_example_app_config());
            mock.expect_save().returning(|| Ok(()));
            mock
        });
        ApplicationManager::new(
            Uuid::new_v4(),
            Box::new(repository_mock),
            Arc::new(Mutex::new(Box::new(realm_client))),
        )
    }
}
