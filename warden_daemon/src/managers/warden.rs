use super::realm::{Realm, RealmDescription};
use super::realm_configuration::RealmConfig;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WardenError {
    #[error("Realm with uuid: '{0}' doesn't exist.")]
    NoSuchRealm(Uuid),
    #[error("Can't destroy the realm: {0}")]
    DestroyFail(String),
}

#[async_trait]
pub trait Warden {
    fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenError>;
    async fn destroy_realm(&mut self, realm_uuid: &Uuid) -> Result<(), WardenError>;
    fn get_realm(
        &mut self,
        realm_uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Realm + Send + Sync>>>, WardenError>;
    async fn inspect_realm(&self, realm_uuid: &Uuid) -> Result<RealmDescription, WardenError>;
    async fn list_realms(&self) -> Vec<RealmDescription>;
}
