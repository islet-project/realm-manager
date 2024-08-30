use std::{collections::HashMap, net::IpAddr};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::application::ApplicationData;

#[derive(Debug, Error, PartialEq, PartialOrd)]
pub enum RealmClientError {
    #[error("Can't connect with the Realm: {0}")]
    RealmConnectionFail(String),
    #[error("Can't communicate with connected Realm: {0}")]
    CommunicationFail(String),
    #[error("Received error from Realm daemon: {0}")]
    RealmDaemonError(String),
    #[error("Invalid response from Realm: {0}")]
    InvalidResponse(String),
    #[error("Realm disconnected.")]
    RealmDisconnection(),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RealmProvisioningConfig {
    pub applications_data: Vec<ApplicationData>,
}

#[async_trait]
pub trait RealmClient {
    async fn provision_applications(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError>;
    async fn start_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn stop_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn read_realm_ifs(&mut self) -> Result<HashMap<String, IpAddr>, RealmClientError>;
    async fn kill_application(&mut self, application_uuid: &Uuid) -> Result<(), RealmClientError>;
    async fn shutdown_realm(&mut self) -> Result<(), RealmClientError>;
    async fn reboot_realm(
        &mut self,
        realm_provisioning_config: RealmProvisioningConfig,
        cid: u32,
    ) -> Result<(), RealmClientError>;
}
