use crate::managers::{application::{Application, ApplicationConfig, ApplicationCreator}, application_manager::ApplicationManager};


pub struct ApplicationFabric {}

impl ApplicationFabric{
    pub fn new() -> Self {
        ApplicationFabric {}
    }
}

impl ApplicationCreator for ApplicationFabric {
    fn create_application(&self, config: ApplicationConfig) -> Box<dyn Application + Send + Sync> {
        Box::new(ApplicationManager::new(config))
    }
}