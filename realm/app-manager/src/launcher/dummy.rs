use std::path::{Path, PathBuf};

use async_trait::async_trait;
use log::debug;
use nix::unistd::{getgid, getuid};
use thiserror::Error;
use tokio::fs;

use super::handler::{ExecConfig, SimpleApplicationHandler};
use super::{ApplicationHandler, ApplicationMetadata, Launcher, Result};

#[derive(Debug, Error)]
pub enum DummyLauncherError {
    #[error("File copy error")]
    FileCopyError(#[source] std::io::Error),

    #[error("File write error")]
    FileWriteError(#[source] std::io::Error),
}

pub struct DummyLauncher {}

const SCRIPT: &str = r#"
while true; do echo "I'm alive"; sleep 1; done
"#;

impl DummyLauncher {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Launcher for DummyLauncher {
    async fn install(&mut self, path: &Path, _: &str, _: &str, _: &str) -> Result<ApplicationMetadata> {
        fs::copy("/usr/bin/busybox", path.join("busybox"))
            .await
            .map_err(DummyLauncherError::FileCopyError)?;

        fs::write(path.join("script.sh"), SCRIPT)
            .await
            .map_err(DummyLauncherError::FileWriteError)?;

        Ok(ApplicationMetadata {
            vendor_data: vec![vec![0x11, 0x22, 0x33]],
            image_hash: vec![1,2,3,4]
        })
    }

    async fn prepare(&mut self, path: &Path) -> Result<Box<dyn ApplicationHandler + Send + Sync>> {
        let config = ExecConfig {
            exec: PathBuf::from("/busybox"),
            argv: vec!["sh".to_owned(), "/script.sh".to_owned()],
            envp: std::env::vars().collect(),
            uid: getuid(),
            gid: getgid(),
            chroot: Some(path.to_owned()),
            chdir: Some(PathBuf::from("/")),
        };

        debug!("Launching from config: {:?}", config);
        let handler = SimpleApplicationHandler::new(config);
        Ok(Box::new(handler))
    }
}
