use super::realm::{Realm, RealmDescription, State};
use super::realm_configuration::RealmConfig;
use super::warden::{RealmCreator, Warden, WardenError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct WardenDaemon {
    realm_fabric: Box<dyn RealmCreator + Send + Sync>,
    realms_managers: HashMap<Uuid, Arc<Mutex<Box<dyn Realm + Send + Sync>>>>,
}

impl WardenDaemon {
    pub fn new(
        realms: HashMap<Uuid, Arc<Mutex<Box<dyn Realm + Send + Sync>>>>,
        rm_fabric: Box<dyn RealmCreator + Send + Sync>,
    ) -> Self {
        WardenDaemon {
            realm_fabric: rm_fabric,
            realms_managers: realms,
        }
    }
}

#[async_trait]
impl Warden for WardenDaemon {
    async fn create_realm(&mut self, config: RealmConfig) -> Result<Uuid, WardenError> {
        let uuid = Uuid::new_v4();
        let _ = self.realms_managers.insert(
            uuid,
            Arc::new(Mutex::new(
                self.realm_fabric.create_realm(uuid, config).await?,
            )),
        );
        Ok(uuid)
    }

    async fn destroy_realm(&mut self, realm_uuid: &Uuid) -> Result<(), WardenError> {
        let realm = self
            .realms_managers
            .get(realm_uuid)
            .ok_or(WardenError::NoSuchRealm(*realm_uuid))?;
        let realm_state = realm.lock().await.get_realm_data();
        if realm_state.state != State::Halted {
            return Err(WardenError::DestroyFail(String::from(
                "Can't destroy realm that isn't stopped.",
            )));
        }
        self.realms_managers
            .remove(realm_uuid)
            .ok_or(WardenError::NoSuchRealm(*realm_uuid))
            .map(|_| ())?;
        self.realm_fabric.clean_up_realm(realm_uuid).await
    }

    async fn list_realms(&self) -> Vec<RealmDescription> {
        let mut vec = vec![];
        for (uuid, realm_manager) in &self.realms_managers {
            vec.push(RealmDescription {
                uuid: *uuid,
                realm_data: realm_manager.lock().await.get_realm_data(),
            });
        }
        vec
    }

    async fn inspect_realm(&self, realm_uuid: &Uuid) -> Result<RealmDescription, WardenError> {
        match self.realms_managers.get(realm_uuid) {
            Some(realm_manager) => Ok(RealmDescription {
                uuid: *realm_uuid,
                realm_data: realm_manager.lock().await.get_realm_data(),
            }),
            None => Err(WardenError::NoSuchRealm(*realm_uuid)),
        }
    }

    fn get_realm(
        &mut self,
        realm_uuid: &Uuid,
    ) -> Result<Arc<Mutex<Box<dyn Realm + Send + Sync>>>, WardenError> {
        self.realms_managers
            .get(realm_uuid)
            .cloned()
            .ok_or(WardenError::NoSuchRealm(*realm_uuid))
    }
}

#[cfg(test)]
mod test {
    use crate::managers::realm::{RealmData, State};
    use crate::utils::test_utilities::{
        create_example_realm_config, create_example_realm_data, MockRealm, MockRealmManagerCreator,
    };

    use super::*;

    #[test]
    fn new() {
        let daemon = create_host_daemon(None);
        assert_eq!(daemon.realms_managers.len(), 0);
    }

    #[tokio::test]
    async fn create_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = daemon
            .create_realm(create_example_realm_config())
            .await
            .unwrap();
        assert!(daemon.realms_managers.contains_key(&uuid));
    }

    #[tokio::test]
    async fn get_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = daemon
            .create_realm(create_example_realm_config())
            .await
            .unwrap();
        assert!(daemon.realms_managers.contains_key(&uuid));
        assert!(daemon.get_realm(&uuid).is_ok());
    }

    #[tokio::test]
    async fn get_none_existing_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = Uuid::new_v4();
        let res = daemon.get_realm(&uuid);
        assert_eq!(res.err(), Some(WardenError::NoSuchRealm(uuid)));
    }

    #[tokio::test]
    async fn destroy_created_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = daemon
            .create_realm(create_example_realm_config())
            .await
            .unwrap();
        assert!(daemon.realms_managers.contains_key(&uuid));
        assert_eq!(daemon.destroy_realm(&uuid).await, Ok(()));
    }

    #[tokio::test]
    async fn destroy_missing_realm() {
        let mut daemon = create_host_daemon(None);
        let uuid = Uuid::new_v4();
        assert_eq!(
            daemon.destroy_realm(&uuid).await,
            Err(WardenError::NoSuchRealm(uuid))
        );
    }

    #[tokio::test]
    async fn destroy_not_halted_realm() {
        let mut mock_realm = MockRealm::new();
        mock_realm.expect_get_realm_data().returning(|| RealmData {
            state: State::Running,
            applications: vec![],
        });
        let mut daemon = create_host_daemon(Some(mock_realm));
        let uuid = daemon
            .create_realm(create_example_realm_config())
            .await
            .unwrap();
        assert_eq!(
            daemon.destroy_realm(&uuid).await,
            Err(WardenError::DestroyFail(String::from(
                "Can't destroy realm that isn't stopped."
            )))
        );
    }

    #[tokio::test]
    async fn inspect_created_realm() {
        let mut realm = MockRealm::new();
        realm
            .expect_get_realm_data()
            .returning(create_example_realm_data);
        let mut daemon = create_host_daemon(Some(realm));
        let uuid = daemon
            .create_realm(create_example_realm_config())
            .await
            .unwrap();
        assert_eq!(
            daemon.inspect_realm(&uuid).await,
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
            daemon.inspect_realm(&uuid).await,
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
            .returning(create_example_realm_data);
        let mut daemon = create_host_daemon(Some(realm));
        let uuid = daemon
            .create_realm(create_example_realm_config())
            .await
            .unwrap();
        let listed_realm = daemon
            .list_realms()
            .await
            .into_iter()
            .find(|descriptor| descriptor.uuid == uuid)
            .take()
            .unwrap();
        assert_eq!(listed_realm.uuid, uuid);
    }

    fn create_host_daemon(realm_mock: Option<MockRealm>) -> WardenDaemon {
        let realm_mock = Box::new(realm_mock.unwrap_or({
            let mut realm_mock = MockRealm::new();
            realm_mock.expect_get_realm_data().returning(|| RealmData {
                state: State::Halted,
                applications: vec![],
            });
            realm_mock
        }));
        let mut creator_mock = MockRealmManagerCreator::new();
        creator_mock
            .expect_create_realm()
            .return_once(move |_, _| Ok(realm_mock));
        creator_mock.expect_clean_up_realm().returning(|_| Ok(()));
        WardenDaemon::new(HashMap::new(), Box::new(creator_mock))
    }
}
