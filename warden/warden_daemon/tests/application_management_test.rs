use client_lib::WardenConnection;
use common::{create_example_realm_config, request_shutdown, WorkdirManager};
use uuid::Uuid;
use warden_client::{application::ApplicationConfig, realm::State};
use warden_daemon::daemon::DaemonBuilder;

mod common;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn manage_realm_and_application() {
    env_logger::init();
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let cli = common::create_example_cli(
        usock_path.clone(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let app = DaemonBuilder::default().build_daemon(cli).await.unwrap();
    let handle = app.run().await.unwrap();

    let mut connection = WardenConnection::connect(usock_path)
        .await
        .expect("Can't connect to created Warden daemon.");

    let realm_config = create_example_realm_config();

    let uuid = connection
        .create_realm(realm_config)
        .await
        .expect("Can't create realm.");

    let realms = connection.list_realms().await.expect("Can't list realms.");

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
        .expect("Can't create application.");

    connection
        .start_realm(uuid)
        .await
        .expect("Can't start realm.");

    assert!(connection.stop_application(uuid, app_uuid).await.is_ok());
    assert!(connection.start_application(uuid, app_uuid).await.is_ok());
    assert!(connection
        .update_application(uuid, app_uuid, application_config.clone())
        .await
        .is_ok());

    assert!(matches!(
        connection
            .inspect_realm(uuid)
            .await
            .expect("Can't inspect realm.")
            .state,
        State::NeedReboot
    ));
    connection
        .reboot_realm(uuid)
        .await
        .expect("Can't reboot realm.");
    connection
        .stop_realm(uuid)
        .await
        .expect("Can't stop realm.");

    application_config.data_storage_size_mb = application_config.data_storage_size_mb / 4;
    application_config.image_storage_size_mb = application_config.image_storage_size_mb / 4;
    assert!(connection
        .update_application(uuid, app_uuid, application_config)
        .await
        .is_ok());

    connection
        .start_realm(uuid)
        .await
        .expect("Can't start realm.");

    request_shutdown();
    assert!(handle.await.unwrap().is_ok());
}
