use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use warden_client::{
    applciation::ApplicationConfig,
    client::WardenDaemonError,
    realm::{RealmConfig, RealmDescription},
};

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WardenClientError {
    #[error("Failed to connect to Warden's socket at path: {socket_path}")]
    ConnectionFailed { socket_path: PathBuf },
    #[error("Warden operation failed: {0}")]
    WardenOperationFail(#[from] WardenDaemonError),
}

pub struct WardenConnection {}

impl Drop for WardenConnection {
    fn drop(&mut self) {
        todo!()
    }
}

impl WardenConnection {
    pub fn connect(warden_socket_path: PathBuf) -> WardenConnection {
        todo!()
    }

    pub async fn create_realm(&self, config: RealmConfig) -> Result<Uuid, WardenClientError> {
        todo!()
    }

    pub async fn start_realm(&self, uuid: Uuid) -> Result<(), WardenClientError> {
        todo!()
    }

    pub async fn stop_realm(&self, uuid: Uuid) -> Result<(), WardenClientError> {
        todo!()
    }

    pub async fn reboot_realm(&self, uuid: Uuid) -> Result<(), WardenClientError> {
        todo!()
    }

    pub async fn destroy_realm(&self, uuid: Uuid) -> Result<(), WardenClientError> {
        todo!()
    }

    pub async fn inspect_realm(&self, uuid: Uuid) -> Result<RealmDescription, WardenClientError> {
        todo!()
    }

    pub async fn list_realms() -> Result<Vec<RealmDescription>, WardenClientError> {
        todo!()
    }

    pub async fn create_application(
        &self,
        realm_uuid: Uuid,
        config: ApplicationConfig,
    ) -> Result<Uuid, WardenClientError> {
        todo!()
    }

    pub async fn start_application(
        &self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
    ) -> Result<(), WardenClientError> {
        todo!()
    }

    pub async fn stop_application(
        &self,
        realm_uuid: Uuid,
        application_uuid: Uuid,
    ) -> Result<(), WardenClientError> {
        todo!()
    }
}
