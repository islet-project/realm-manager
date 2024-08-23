use common::PathResourceManager;
use nix::{
    sys::signal::{
        self,
        Signal::{SIGINT, SIGTERM},
    },
    unistd::Pid,
};
use warden_daemon::daemon::Daemon;

mod common;

#[tokio::test]
#[ignore]
async fn sig_term_shutdown() {
    let usock_path_manager = PathResourceManager::new().await;
    let workdir_path_manager = PathResourceManager::new().await;
    let cli = common::create_example_cli(
        usock_path_manager.get_path().to_path_buf(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let app = Daemon::new(cli).await.unwrap();
    let handle = app.run().await.unwrap();
    signal::kill(Pid::this(), SIGTERM).unwrap();
    assert!(handle.await.unwrap().is_ok());
}

#[tokio::test]
#[ignore]
async fn sig_int_shutdown() {
    let usock_path_manager = PathResourceManager::new().await;
    let workdir_path_manager = PathResourceManager::new().await;
    let cli = common::create_example_cli(
        usock_path_manager.get_path().to_path_buf(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let app = Daemon::new(cli).await.unwrap();
    let handle = app.run().await.unwrap();
    signal::kill(Pid::this(), SIGINT).unwrap();
    assert!(handle.await.unwrap().is_ok());
}
