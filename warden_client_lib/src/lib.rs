use request_handlers::{
    connect_to_warden_socket, create_application, create_realm, destroy_realm, inspect_realm,
    list_realms, reboot_realm, start_application, start_realm, stop_application, stop_realm,
    update_application,
};
use std::path::PathBuf;
use tokio::net::UnixStream;
use utils::serde::JsonFramed;
use uuid::Uuid;
use warden_client::{
    applciation::ApplicationConfig,
    realm::{RealmConfig, RealmDescription},
    warden::{WardenCommand, WardenResponse},
};
use warden_client_error::WardenClientError;
mod request_handlers;
mod warden_client_error;

pub struct WardenConnection {
    communicator: JsonFramed<UnixStream, WardenResponse, WardenCommand>,
}

impl WardenConnection {
    pub async fn connect(
        warden_socket_path: PathBuf,
    ) -> Result<WardenConnection, WardenClientError> {
        let stream = connect_to_warden_socket(warden_socket_path).await?;
        Ok(WardenConnection {
            communicator: JsonFramed::<UnixStream, WardenResponse, WardenCommand>::new(stream),
        })
    }

    pub async fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenClientError> {
        create_realm(&mut self.communicator, config).await
    }

    pub async fn start_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        start_realm(&mut self.communicator, uuid).await
    }

    pub async fn stop_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        stop_realm(&mut self.communicator, uuid).await
    }

    pub async fn reboot_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        reboot_realm(&mut self.communicator, uuid).await
    }

    pub async fn destroy_realm(&mut self, uuid: Uuid) -> Result<(), WardenClientError> {
        destroy_realm(&mut self.communicator, uuid).await
    }

    pub async fn inspect_realm(
        &mut self,
        uuid: Uuid,
    ) -> Result<RealmDescription, WardenClientError> {
        inspect_realm(&mut self.communicator, uuid).await
    }

    pub async fn list_realms(&mut self) -> Result<Vec<RealmDescription>, WardenClientError> {
        list_realms(&mut self.communicator).await
    }

    pub async fn create_application(
        &mut self,
        realm_uuid: Uuid,
        config: ApplicationConfig,
    ) -> Result<Uuid, WardenClientError> {
        create_application(&mut self.communicator, realm_uuid, config).await
    }

    pub async fn update_application(
        &mut self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
        config: ApplicationConfig,
    ) -> Result<(), WardenClientError> {
        update_application(&mut self.communicator, realm_uuid, application_uuid, config).await
    }

    pub async fn start_application(
        &mut self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
    ) -> Result<(), WardenClientError> {
        start_application(&mut self.communicator, realm_uuid, application_uuid).await
    }

    pub async fn stop_application(
        &mut self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
    ) -> Result<(), WardenClientError> {
        stop_application(&mut self.communicator, realm_uuid, application_uuid).await
    }
}
