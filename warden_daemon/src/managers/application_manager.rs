use super::{
    application::{Application, ApplicationConfig, ApplicationError},
    realm_client::RealmClient,
};

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct ApplicationManager {
    uuid: Uuid,
    _config: ApplicationConfig,
    realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    new_config: Option<ApplicationConfig>,
}

impl ApplicationManager {
    pub fn new(
        uuid: Uuid,
        config: ApplicationConfig,
        realm_client_handler: Arc<Mutex<Box<dyn RealmClient + Send + Sync>>>,
    ) -> Self {
        ApplicationManager {
            uuid,
            _config: config,
            realm_client_handler,
            new_config: None,
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
            .map_err(|err| ApplicationError::ApplicationStopFail(err.to_string()))?;
        Ok(())
    }
    async fn start(&mut self) -> Result<(), ApplicationError> {
        self.realm_client_handler
            .lock()
            .await
            .start_application(&self.uuid)
            .await
            .map_err(|err| ApplicationError::ApplicationStartFail(err.to_string()))?;
        Ok(())
    }

    fn update(&mut self, config: ApplicationConfig) {
        self.new_config = Some(config);
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::test_utilities::MockRealmClient;
    use crate::{
        managers::{
            application::{Application, ApplicationConfig, ApplicationError},
            realm_client::RealmClientError,
        },
        test_utilities::create_example_app_config,
    };

    use super::ApplicationManager;

    #[test]
    fn new() {
        let application_manager = create_application_manager(None);
        assert_eq!(application_manager.new_config, None);
    }

    #[tokio::test]
    async fn stop() {
        let mut application_manager = create_application_manager(None);
        assert!(application_manager.stop().await.is_ok());
    }

    #[tokio::test]
    async fn stop_fail() {
        let mut realm_client = MockRealmClient::new();
        realm_client
            .expect_stop_application()
            .returning(|_| Err(RealmClientError::RealmConnectorError(String::from(""))));
        let mut application_manager = create_application_manager(Some(realm_client));
        assert_eq!(
            application_manager.stop().await,
            Err(ApplicationError::ApplicationStopFail(
                RealmClientError::RealmConnectorError(String::from("")).to_string()
            ))
        );
    }

    #[tokio::test]
    async fn start() {
        let mut application_manager = create_application_manager(None);
        assert!(application_manager.start().await.is_ok());
    }

    #[tokio::test]
    async fn start_fail() {
        let mut realm_client = MockRealmClient::new();
        realm_client
            .expect_start_application()
            .returning(|_| Err(RealmClientError::RealmConnectorError(String::from(""))));
        let mut application_manager = create_application_manager(Some(realm_client));
        assert_eq!(
            application_manager.start().await,
            Err(ApplicationError::ApplicationStartFail(
                RealmClientError::RealmConnectorError(String::from("")).to_string()
            ))
        );
    }

    #[test]
    fn update() {
        let mut application_manager = create_application_manager(None);
        let app_config = create_example_app_config();
        application_manager.update(app_config.clone());
        assert_eq!(application_manager.new_config, Some(app_config));
    }

    fn create_application_manager(realm_client: Option<MockRealmClient>) -> ApplicationManager {
        let realm_client = realm_client.unwrap_or({
            let mut realm_client = MockRealmClient::new();
            realm_client
                .expect_start_application()
                .returning(|_| Ok(()));
            realm_client.expect_stop_application().returning(|_| Ok(()));
            realm_client
        });
        ApplicationManager::new(
            Uuid::new_v4(),
            ApplicationConfig {},
            Arc::new(Mutex::new(Box::new(realm_client))),
        )
    }
}
