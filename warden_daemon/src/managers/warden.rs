use super::realm::{Realm, RealmDescription};
use super::realm_configuration::RealmConfig;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, PartialOrd)]
pub enum WardenError {
    #[error("Realm with uuid: '{0}' doesn't exist")]
    NoSuchRealm(Uuid),
}

pub trait Warden {
    fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenError>;
    fn destroy_realm(&mut self, realm_uuid: Uuid) -> Result<(), WardenError>;
    fn list_realms(&self) -> Vec<RealmDescription>;
    fn inspect_realm(&self, realm_uuid: Uuid) -> Result<RealmDescription, WardenError>;
    fn get_realm(
        &mut self,
        realm_uuid: &Uuid,
    ) -> Result<&mut Box<dyn Realm + Send + Sync>, WardenError>;
}
