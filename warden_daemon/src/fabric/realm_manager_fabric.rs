use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::managers::application::ApplicationCreator;
use crate::managers::realm::{Realm, RealmCreator};
use crate::client_handler::realm_client_handler::RealmClientHandler;
use crate::managers::realm_configuration::RealmConfig;
use crate::managers::realm_manager::RealmManager;
use crate::socket::vsocket_server::VSockServer;
use crate::virtualization::qemu_runner::QemuRunner;

pub struct RealmManagerFabric {
    qemu_path: PathBuf,
    vsock_server: Arc<Mutex<VSockServer>>,
    application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
}

impl RealmManagerFabric {
    pub fn new(
        qemu_path: PathBuf,
        vsock_server: Arc<Mutex<VSockServer>>,
        application_fabric: Arc<Box<dyn ApplicationCreator + Send + Sync>>,
    ) -> Self {
        RealmManagerFabric {
            qemu_path,
            vsock_server,
            application_fabric,
        }
    }
}

impl RealmCreator for RealmManagerFabric {
    fn create_realm(&self, config: RealmConfig) -> Box<dyn Realm + Send + Sync> {
        Box::new(RealmManager::new(
            config,
            Box::new(QemuRunner::new(self.qemu_path.clone())),
            Arc::new(Mutex::new(Box::new(RealmClientHandler::new(
                self.vsock_server.clone(),
            )))),
            self.application_fabric.clone(),
        ))
    }
}
