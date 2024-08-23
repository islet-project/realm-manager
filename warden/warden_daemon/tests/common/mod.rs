use std::{
    env,
    fs::{remove_dir_all, remove_file},
    path::{Path, PathBuf},
    str::FromStr,
};

use nix::{
    sys::signal::{self, Signal::SIGINT},
    unistd::Pid,
};
use tokio_vsock::VMADDR_CID_HOST;
use uuid::Uuid;
use warden_daemon::cli::Cli;

pub struct PathResourceManager {
    resource_path: Box<dyn AsRef<Path>>,
}

impl PathResourceManager {
    pub async fn new() -> Self {
        const TEST_FOLDER_PATH: &str = "/tmp/warden-daemon-integration-tests";
        tokio::fs::create_dir_all(TEST_FOLDER_PATH).await.unwrap();
        Self {
            resource_path: Box::new(format!("{}/{}", TEST_FOLDER_PATH, Uuid::new_v4())),
        }
    }

    pub fn get_path(&self) -> &Path {
        (*self.resource_path).as_ref()
    }
}

impl Drop for PathResourceManager {
    fn drop(&mut self) {
        let path = self.get_path();
        if let Err(_) = remove_dir_all(path) {
            let _ = remove_file(path);
        }
    }
}

pub fn request_shutdown() {
    signal::kill(Pid::this(), SIGINT).unwrap();
}

pub fn get_kernel_path() -> PathBuf {
    const REALM_KERNEL_PATH_ENV: &str = "REALM_KERNEL_PATH";
    PathBuf::from_str(
        &env::var(REALM_KERNEL_PATH_ENV)
            .expect(&format!("Missing env var: {}", REALM_KERNEL_PATH_ENV)),
    )
    .unwrap()
}

pub fn create_example_cli(unix_sock_path: PathBuf, warden_workdir_path: PathBuf) -> Cli {
    const REALM_QEMU_PATH_ENV: &str = "REALM_QEMU_PATH";
    Cli {
        cid: VMADDR_CID_HOST,
        port: 1337,
        qemu_path: PathBuf::from_str(
            &env::var(REALM_QEMU_PATH_ENV)
                .expect(&format!("Missing env var: {}", REALM_QEMU_PATH_ENV)),
        )
        .unwrap(),
        unix_sock_path,
        warden_workdir_path,
        realm_connection_wait_time_secs: 60,
    }
}
