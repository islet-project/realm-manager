use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    application::{Application, ApplicationConfig, ApplicationError},
    realm_client::RealmClient,
};

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
            .map_err(|err| ApplicationError::ApplicationStopError(format!("{}", err)))?;
        Ok(())
    }
    async fn start(&mut self) -> Result<(), ApplicationError> {
        self.realm_client_handler
            .lock()
            .await
            .start_application(&self.uuid)
            .await
            .map_err(|err| ApplicationError::ApplicationStartFail(format!("{}", err)))?;
        Ok(())
    }

    fn update(&mut self, config: ApplicationConfig) {
        self.new_config = Some(config);
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use async_trait::async_trait;
    use mockall::mock;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::managers::{
        application::{Application, ApplicationConfig, ApplicationError},
        realm_client::{RealmClient, RealmClientError, RealmProvisioningConfig},
    };

    use super::ApplicationManager;

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
            Err(ApplicationError::ApplicationStopError(
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
        application_manager.update(ApplicationConfig {});
        assert!(application_manager.new_config.is_some());
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

    mock! {
        pub RealmClient {}

        #[async_trait]
        impl RealmClient for RealmClient {
            async fn send_realm_provisioning_config(
                &mut self,
                realm_provisioning_config: RealmProvisioningConfig,
                cid: u32,
            ) -> Result<(), RealmClientError>;
            async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
            async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
        }
    }
}
