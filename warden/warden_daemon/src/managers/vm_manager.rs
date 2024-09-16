use std::process::ExitStatus;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum VmManagerError {
    #[error("Unable to launch Vm: {0}")]
    Launch(String),
    #[error("To stop realm's vm you need to launch it first.")]
    VmNotLaunched,
    #[error("Vm already has been launched.")]
    VmAlreadyLaunched,
    #[error("Unable to get realm's exit code: {0}")]
    GetExitCode(String),
    #[error("Unable to shutdown realm's vm: {0}")]
    Shutdown(String),
}

#[async_trait]
pub trait VmManager {
    async fn launch_vm(&mut self, application_uuids: &[&Uuid]) -> Result<(), VmManagerError>;
    async fn shutdown(&mut self) -> Result<(), VmManagerError>;
    fn try_get_exit_status(&mut self) -> Result<Option<ExitStatus>, VmManagerError>;
}
