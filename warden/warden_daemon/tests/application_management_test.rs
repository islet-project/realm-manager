use client_lib::WardenConnection;
use common::{get_kernel_path, request_shutdown, ResourceManager};
use warden_client::{
    application::ApplicationConfig,
    realm::{CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig, State},
};
use warden_daemon::app::App;

mod common;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn manage_realm_and_application() {
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
            kernel_path: get_kernel_path(),
        },
    };

    let uuid = connection.create_realm(realm_config).await.unwrap();

    let realms = connection.list_realms().await.unwrap();

    assert_eq!(realms.len(), 1);
    assert_eq!(realms[0].uuid, uuid);
    assert_eq!(realms[0].applications.len(), 0);

    let mut application_config = ApplicationConfig {
        name: "app_test".to_string(),
        version: "0.0.1".to_string(),
        image_registry: "https://github.com/islet-project/realm-manager".to_string(),
        image_storage_size_mb: 128,
        data_storage_size_mb: 128,
    };

    let app_uuid = connection
        .create_application(uuid, application_config.clone())
        .await
        .unwrap();

    connection.start_realm(uuid).await.unwrap();

    assert!(connection.stop_application(uuid, app_uuid).await.is_ok());
    assert!(connection.start_application(uuid, app_uuid).await.is_ok());
    assert!(connection
        .update_application(uuid, app_uuid, application_config.clone())
        .await
        .is_ok());

    assert!(matches!(
        connection.inspect_realm(uuid).await.unwrap().state,
        State::NeedReboot
    ));
    connection.reboot_realm(uuid).await.unwrap();
    connection.stop_realm(uuid).await.unwrap();

    application_config.data_storage_size_mb = application_config.data_storage_size_mb / 4;
    application_config.image_storage_size_mb = application_config.image_storage_size_mb / 4;
    assert!(connection
        .update_application(uuid, app_uuid, application_config)
        .await
        .is_ok());

    connection.start_realm(uuid).await.unwrap();

    request_shutdown();
    assert!(handle.await.unwrap().is_ok());
}
