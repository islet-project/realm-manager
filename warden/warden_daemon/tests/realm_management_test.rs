mod common;
use client_lib::WardenConnection;
use common::{create_example_realm_config, request_shutdown, WorkdirManager};
use uuid::Uuid;
use warden_client::realm::State;
use warden_daemon::daemon::Daemon;

#[tokio::test]
#[ignore]
async fn manage_realm() {
    env_logger::init();
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let cli = common::create_example_cli(
        usock_path.clone(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let app = Daemon::new(cli).await.unwrap();
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
    assert!(matches!(
        connection
            .inspect_realm(uuid)
            .await
            .expect("Can't inspect realm.")
            .state,
        State::Halted
    ));

    assert!(connection.start_realm(uuid).await.is_ok());
    assert!(matches!(
        connection
            .inspect_realm(uuid)
            .await
            .expect("Can't inspect realm.")
            .state,
        State::Running
    ));

    assert!(connection.reboot_realm(uuid).await.is_ok());
    assert!(matches!(
        connection
            .inspect_realm(uuid)
            .await
            .expect("Can't inspect realm.")
            .state,
        State::Running
    ));

    assert!(connection.stop_realm(uuid).await.is_ok());
    assert!(matches!(
        connection
            .inspect_realm(uuid)
            .await
            .expect("Can't inspect realm.")
            .state,
        State::Halted
    ));

    assert!(connection.destroy_realm(uuid).await.is_ok());
    assert!(connection.inspect_realm(uuid).await.is_err());

    request_shutdown();
    assert!(handle.await.unwrap().is_ok());
}
