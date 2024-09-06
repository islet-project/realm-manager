use std::process::ExitStatus;

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum VmManagerError {
    #[error("Unable to create realm's vm: {0}")]
    Create(String),
    #[error("Unable to launch Vm: {0}")]
    Launch(#[from] std::io::Error),
    #[error("To stop realm's vm you need to launch it first.")]
    VmNotLaunched,
    #[error("Unable to stop realm's vm.")]
    Stop,
    #[error("Unable to destroy realm's vm: {0}")]
    Destroy(String),
}

pub trait VmManager {
    fn delete_vm(&mut self) -> Result<(), VmManagerError>;
    fn get_exit_status(&mut self) -> Option<ExitStatus>;
    fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError>;
    fn stop_vm(&mut self) -> Result<(), VmManagerError>;
}
