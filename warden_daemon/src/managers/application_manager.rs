use super::application::{Application, ApplicationConfig, ApplicationError};

pub struct ApplicationManager {}

impl ApplicationManager {
    pub fn new(config: ApplicationConfig) -> Self {
        ApplicationManager {}
    }
}

impl Application for ApplicationManager {
    fn stop(&mut self) -> Result<(), ApplicationError> {
        todo!()
    }
    fn start(&mut self) -> Result<(), ApplicationError> {
        todo!()
    }
    fn update(&mut self) -> Result<(), ApplicationError> {
        todo!()
    }
}
