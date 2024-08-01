use std::{path::Path, process::ExitStatus};

use async_trait::async_trait;
use dummy::DummyLauncherError;
use handler::ApplicationHandlerError;
use thiserror::Error;

pub mod dummy;
pub mod handler;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("Applicatino handler error")]
    HandlerError(#[from] ApplicationHandlerError),

    #[error("Dummy launcher error")]
    DummyLauncherError(#[from] DummyLauncherError)
}

pub type Result<T> = std::result::Result<T, LauncherError>;

#[async_trait]
pub trait ApplicationHandler {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn kill(&mut self) -> Result<()>;
    async fn wait(&mut self) -> Result<ExitStatus>;
    async fn try_wait(&mut self) -> Result<Option<ExitStatus>>;
}

#[async_trait]
pub trait Launcher {
    async fn install(&mut self, path: &Path, name: &str, version: &str) -> Result<()>;
    async fn read_vendor_data(&self, path: &Path) -> Result<Vec<Vec<u8>>>;
    async fn prepare(&mut self, path: &Path) -> Result<Box<dyn ApplicationHandler + Send + Sync>>;
}

