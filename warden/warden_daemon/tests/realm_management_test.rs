mod common;
use client_lib::WardenConnection;
use common::{create_example_realm_config, request_shutdown, PathResourceManager};
use warden_client::realm::State;
use warden_daemon::app::App;

#[tokio::test]
#[ignore]
async fn manage_realm() {
    env_logger::init();
    let usock_path_manager = PathResourceManager::new().await;
    let workdir_path_manager = PathResourceManager::new().await;
    let cli = common::create_example_cli(
        usock_path_manager.get_path().to_path_buf(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let app = App::new(cli).await.unwrap();
    let handle = app.run().await.unwrap();

    let mut connection = WardenConnection::connect(usock_path_manager.get_path().to_path_buf())
        .await
        .expect("Can't connect to created Warden daemon.");

    let realm_config = create_example_realm_config();

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
