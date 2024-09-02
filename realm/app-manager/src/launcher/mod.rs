use std::path::Path;
use std::process::ExitStatus;

use async_trait::async_trait;

pub mod dummy;
pub mod handler;
pub mod oci;

use crate::error::Result;

#[async_trait]
pub trait ApplicationHandler {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn kill(&mut self) -> Result<()>;
    async fn try_wait(&mut self) -> Result<Option<ExitStatus>>;
}

#[async_trait]
pub trait Launcher {
    async fn install(
        &mut self,
        path: &Path,
        im_url: &str,
        name: &str,
        version: &str,
    ) -> Result<Vec<Vec<u8>>>;
    async fn prepare(&mut self, path: &Path) -> Result<Box<dyn ApplicationHandler + Send + Sync>>;
}
