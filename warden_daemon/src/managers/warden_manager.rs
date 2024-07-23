use super::realm::{Realm, RealmCreator, RealmDescription, State};
use super::realm_configuration::RealmConfig;
use super::warden::{Warden, WardenError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct WardenDaemon {
    managers_fabric: Box<dyn RealmCreator + Send + Sync>,
    managers_map: HashMap<Uuid, Arc<Mutex<Box<dyn Realm + Send + Sync>>>>,
}

impl WardenDaemon {
    pub fn new(rm_fabric: Box<dyn RealmCreator + Send + Sync>) -> Self {
        WardenDaemon {
            managers_fabric: rm_fabric,
            managers_map: HashMap::new(),
        }
    }
}

#[async_trait]
impl Warden for WardenDaemon {
    fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenError> {
        let uuid = Uuid::new_v4();
        let _ = self.managers_map.insert(
            uuid,
            Arc::new(Mutex::new(self.managers_fabric.create_realm(config))),
        );
        Ok(uuid)
    }

    async fn destroy_realm(&mut self, realm_uuid: Uuid) -> Result<(), WardenError> {
        let realm = self
            .managers_map
            .get(&realm_uuid)
            .ok_or(WardenError::NoSuchRealm(realm_uuid))?;
        let realm_state = realm.lock().await.get_realm_data();
        if realm_state.state != State::Halted {
            return Err(WardenError::DestroyFail(String::from(
                "Can't destroy realm that isn't stopped!",
            )));
        }
        self.managers_map
            .remove(&realm_uuid)
            .ok_or(WardenError::NoSuchRealm(realm_uuid))
            .map(|_| ())
    }

    async fn list_realms(&self) -> Vec<RealmDescription> {
        let mut vec = vec![];
        for (uuid, realm_manager) in &self.managers_map {
            vec.push(RealmDescription {
                uuid: *uuid,
                realm_data: realm_manager.lock().await.get_realm_data(),
            });
        }
        vec
    }

    async fn inspect_realm(&self, realm_uuid: Uuid) -> Result<RealmDescription, WardenError> {
        match self.managers_map.get(&realm_uuid) {
            Some(realm_manager) => Ok(RealmDescription {
                uuid: realm_uuid,
                realm_data: realm_manager.lock().await.get_realm_data(),
            }),
            None => Err(WardenError::NoSuchRealm(realm_uuid)),
        }
    }

    fn get_realm(
        &mut self,
        realm_uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Realm + Send + Sync>>>, WardenError> {
        self.managers_map
            .get(realm_uuid)
            .cloned()
            .ok_or(WardenError::NoSuchRealm(*realm_uuid))
    }
}

#[cfg(test)]
mod test {
    use crate::managers::realm::{RealmData, State};
    use crate::test_utilities::{
        create_example_realm_config, create_example_realm_data, MockRealm, MockRealmManagerCreator,
    };

    use super::*;

    #[test]
    fn test_create_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = daemon.create_realm(create_example_realm_config()).unwrap();
        assert!(daemon.managers_map.contains_key(&uuid));
    }

    #[tokio::test]
    async fn destroy_created_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = daemon.create_realm(create_example_realm_config()).unwrap();
        assert!(daemon.managers_map.contains_key(&uuid));

        assert_eq!(daemon.destroy_realm(uuid).await, Ok(()));
    }

    #[tokio::test]
    async fn destroy_missing_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = Uuid::new_v4();
        assert_eq!(
            daemon.destroy_realm(uuid.clone()).await,
            Err(WardenError::NoSuchRealm(uuid))
        );
    }

    #[tokio::test]
    async fn destroy_not_halted_realm() {
        let mut mock_realm = MockRealm::new();
        mock_realm.expect_get_realm_data().returning(|| RealmData {
            state: State::Running,
        });
        let mut daemon = create_host_daemon(Some(mock_realm));
        let uuid = daemon.create_realm(create_example_realm_config()).unwrap();
        assert_eq!(
            daemon.destroy_realm(uuid.clone()).await,
            Err(WardenError::DestroyFail(String::from(
                "Can't destroy realm that isn't stopped!"
            )))
        );
    }

    #[tokio::test]
    async fn inspect_created_realm() {
        let mut realm = MockRealm::new();
        realm
            .expect_get_realm_data()
            .returning(|| create_example_realm_data());
        let mut daemon = create_host_daemon(Some(realm));
        let uuid = daemon.create_realm(create_example_realm_config()).unwrap();
        assert_eq!(
            daemon.inspect_realm(uuid).await,
            Ok(RealmDescription {
                uuid,
                realm_data: create_example_realm_data()
            })
        );
    }

    #[tokio::test]
    async fn inspect_not_existing_realm() {
        let daemon = create_host_daemon(None);
        let uuid = Uuid::new_v4();
        assert_eq!(
            daemon.inspect_realm(uuid).await,
            Err(WardenError::NoSuchRealm(uuid))
        );
    }

    #[tokio::test]
    async fn list_newly_created_warden() {
        let daemon = create_host_daemon(None);
        let listed_realms = daemon.list_realms().await;
        assert!(listed_realms.is_empty());
    }

    #[tokio::test]
    async fn list_realms() {
        let mut realm = MockRealm::new();
        realm
            .expect_get_realm_data()
            .returning(|| create_example_realm_data());
        let mut daemon = create_host_daemon(Some(realm));
        let uuid = daemon.create_realm(create_example_realm_config()).unwrap();
        let listed_realm = daemon
            .list_realms()
            .await
            .into_iter()
            .find(|descriptor| descriptor.uuid == uuid)
            .take()
            .unwrap();
        assert_eq!(listed_realm.uuid, uuid);
    }

    #[test]
    fn get_none_existing_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = Uuid::new_v4();
        let res = daemon.get_realm(&uuid);
        assert_eq!(res.err(), Some(WardenError::NoSuchRealm(uuid)));
    }

    fn create_host_daemon(realm_mock: Option<MockRealm>) -> WardenDaemon {
        let realm_mock = Box::new(realm_mock.unwrap_or({
            let mut realm_mock = MockRealm::new();
            realm_mock.expect_get_realm_data().returning(|| RealmData {
                state: State::Halted,
            });
            realm_mock
        }));
        let mut creator_mock = MockRealmManagerCreator::new();
        creator_mock
            .expect_create_realm()
            .return_once(move |_| realm_mock);
        WardenDaemon::new(Box::new(creator_mock))
    }
}
