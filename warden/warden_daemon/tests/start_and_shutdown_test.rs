use common::WorkdirManager;
use nix::{
    sys::signal::{
        self,
        Signal::{SIGINT, SIGTERM},
    },
    unistd::Pid,
};
use uuid::Uuid;
use warden_daemon::daemon::Daemon;

mod common;

#[tokio::test]
#[ignore]
async fn sig_term_shutdown() {
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let cli = common::create_example_cli(usock_path, workdir_path_manager.get_path().to_path_buf());
    let app = Daemon::new(cli).await.unwrap();
    let handle = app.run().await.unwrap();
    signal::kill(Pid::this(), SIGTERM).unwrap();
    assert!(handle.await.unwrap().is_ok());
}

#[tokio::test]
#[ignore]
async fn sig_int_shutdown() {
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let cli = common::create_example_cli(usock_path, workdir_path_manager.get_path().to_path_buf());
    let app = Daemon::new(cli).await.unwrap();
    let handle = app.run().await.unwrap();
    signal::kill(Pid::this(), SIGINT).unwrap();
    assert!(handle.await.unwrap().is_ok());
}
