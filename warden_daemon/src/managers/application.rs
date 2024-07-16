pub enum ApplicationError {}

pub trait ApplicationCreator {
    fn create_application(&self, config: ApplicationConfig) -> Box<dyn Application + Send + Sync>;
}

pub trait Application {
    fn stop(&mut self) -> Result<(), ApplicationError>;
    fn start(&mut self) -> Result<(), ApplicationError>;
    fn update(&mut self) -> Result<(), ApplicationError>;
}

pub struct ApplicationConfig {}
