mod common;
use client_lib::WardenConnection;
use common::{create_example_realm_config, request_shutdown, WorkdirManager};
use ipnet::IpNet;
use uuid::Uuid;
use warden_client::realm::State;
use warden_daemon::daemon::DaemonBuilder;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn check_realms_network() {
    env_logger::init();
    let workdir_path_manager = WorkdirManager::new().await;
    let usock_path = workdir_path_manager
        .get_path()
        .join(format!("usock-{}", Uuid::new_v4()));
    let cli = common::create_example_cli(
        usock_path.clone(),
        workdir_path_manager.get_path().to_path_buf(),
    );
    let bridge_ip = cli.network_address.clone();
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

    let realm_desc = connection
        .inspect_realm(uuid)
        .await
        .expect("Can't inspect realm.");

    assert_eq!(realm_desc.network.len(), 0);
    assert!(matches!(realm_desc.state, State::Halted));

    assert!(connection.start_realm(uuid).await.is_ok());

    let realm_desc = connection
        .inspect_realm(uuid)
        .await
        .expect("Can't inspect realm.");

    assert_eq!(realm_desc.network.len(), 1);
    assert_eq!(
        bridge_ip.network(),
        IpNet::new(realm_desc.network[0].ip, bridge_ip.prefix_len())
            .unwrap()
            .network()
    );
    assert!(matches!(realm_desc.state, State::Running));

    assert!(connection.stop_realm(uuid).await.is_ok());
    assert!(connection.destroy_realm(uuid).await.is_ok());

    request_shutdown();
    assert!(handle.await.unwrap().is_ok());
}
