use std::net::Ipv4Addr;

use common::WorkdirManager;
use ipnet::{IpNet, Ipv4Net};
use nix::{
    sys::signal::{
        self,
        Signal::{SIGINT, SIGTERM},
    },
    unistd::Pid,
};
use uuid::Uuid;
use warden_daemon::daemon::DaemonBuilder;

mod common;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn sig_term_shutdown() {
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let cli = common::create_example_cli(usock_path, workdir_path_manager.get_path().to_path_buf());
    let app = DaemonBuilder::build(cli).await.unwrap();
    let handle = app.run().await.unwrap();
    signal::kill(Pid::this(), SIGTERM).unwrap();
    assert!(handle.await.unwrap().is_ok());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn sig_int_shutdown() {
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let mut cli =
        common::create_example_cli(usock_path, workdir_path_manager.get_path().to_path_buf());
    cli.network_address = IpNet::V4(Ipv4Net::new(Ipv4Addr::new(192, 168, 100, 0), 24).unwrap());
    cli.bridge_name = String::from("BrigeTest2");
    let app = DaemonBuilder::build(cli).await.unwrap();
    let handle = app.run().await.unwrap();
    signal::kill(Pid::this(), SIGINT).unwrap();
    assert!(handle.await.unwrap().is_ok());
}
