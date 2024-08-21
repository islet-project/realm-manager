use client_lib::WardenConnection;
use common::{request_shutdown, ResourceManager};
use warden_client::realm::{
    CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig, State,
};
use warden_daemon::app::App;

mod common;

#[tokio::test]
#[ignore]
async fn manage_realm() {
    env_logger::init();
    let usock_path_manager = ResourceManager::new();
    let workdir_path_manager = ResourceManager::new();
    let cli = common::create_example_cli(
        usock_path_manager.get_path().to_path_buf(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let app = App::new(cli).await.unwrap();
    let handle = app.run().await.unwrap();

    let mut connection = WardenConnection::connect(usock_path_manager.get_path().to_path_buf())
        .await
        .unwrap();

    let realm_config = RealmConfig {
        machine: "virt".to_string(),
        cpu: CpuConfig {
            cpu: "cortex-a57".to_string(),
            cores_number: 2,
        },
        memory: MemoryConfig { ram_size: 2048 },
        network: NetworkConfig {
            vsock_cid: 12344,
            tap_device: "tap200".to_string(),
            mac_address: "52:55:00:d1:55:01".to_string(),
            hardware_device: Some("e1000".to_string()),
            remote_terminal_uri: None,
        },
        kernel: KernelConfig {
            kernel_path: common::get_kernel_path(),
        },
    };

    let uuid = connection.create_realm(realm_config).await.unwrap();

    let realms = connection.list_realms().await.unwrap();

    assert_eq!(realms.len(), 1);
    assert_eq!(realms[0].uuid, uuid);
    assert!(matches!(
        connection.inspect_realm(uuid).await.unwrap().state,
        State::Halted
    ));

    assert!(connection.start_realm(uuid).await.is_ok());
    assert!(matches!(
        connection.inspect_realm(uuid).await.unwrap().state,
        State::Running
    ));

    assert!(connection.reboot_realm(uuid).await.is_ok());
    assert!(matches!(
        connection.inspect_realm(uuid).await.unwrap().state,
        State::Running
    ));

    assert!(connection.stop_realm(uuid).await.is_ok());
    assert!(matches!(
        connection.inspect_realm(uuid).await.unwrap().state,
        State::Halted
    ));

    assert!(connection.destroy_realm(uuid).await.is_ok());
    assert!(connection.inspect_realm(uuid).await.is_err());

    request_shutdown();
    assert!(handle.await.unwrap().is_ok());
}
