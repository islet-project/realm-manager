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

pub struct ResourceManager {
    resource_path: PathBuf,
}
impl ResourceManager {
    pub fn new() -> Self {
        let uuid = Uuid::new_v4();
        Self {
            resource_path: PathBuf::from_str(&format!("/tmp/{}", uuid.to_string())).unwrap(),
        }
    }
    pub fn get_path(&self) -> &Path {
        self.resource_path.as_path()
    }
}

impl Drop for ResourceManager {
    fn drop(&mut self) {
        let _ = remove_file(&self.resource_path);
        let _ = remove_dir_all(&self.resource_path);
    }
}

pub fn request_shutdown() {
    signal::kill(Pid::this(), SIGINT).unwrap();
}

pub fn get_kernel_path() -> PathBuf {
    const REALM_KERNEL_PATH_ENV: &str = "REALM_KERNEL_PATH";
    PathBuf::from_str(&env::var(REALM_KERNEL_PATH_ENV).unwrap()).unwrap()
}

pub fn create_example_cli(unix_sock_path: PathBuf, warden_workdir_path: PathBuf) -> Cli {
    const REALM_QEMU_PATH_ENV: &str = "REALM_QEMU_PATH";
    Cli {
        cid: VMADDR_CID_HOST,
        port: 1337,
        qemu_path: PathBuf::from_str(&env::var(REALM_QEMU_PATH_ENV).unwrap()).unwrap(),
        unix_sock_path,
        warden_workdir_path,
        realm_connection_wait_time_secs: 60,
    }
}
