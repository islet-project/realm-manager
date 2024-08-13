use std::path::PathBuf;

use tokio::net::UnixStream;
use utils::serde::json_framed::JsonFramed;
use uuid::Uuid;
use warden_client::{
    application::ApplicationConfig,
    realm::{RealmConfig, RealmDescription},
    warden::{WardenCommand, WardenResponse},
};

use crate::warden_client_error::WardenClientError;

type Communicator = JsonFramed<UnixStream, WardenResponse, WardenCommand>;

pub async fn create_realm(
    communicator: &mut Communicator,
    config: RealmConfig,
) -> Result<Uuid, WardenClientError> {
    match communicate(communicator, WardenCommand::CreateRealm { config }).await? {
        WardenResponse::CreatedRealm { uuid } => Ok(uuid),
        response => Err(handle_error_response(response)),
    }
}

pub async fn start_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::StartRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn stop_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::StopRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn reboot_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::RebootRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn destroy_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(communicator, WardenCommand::DestroyRealm { uuid }).await? {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn inspect_realm(
    communicator: &mut Communicator,
    uuid: Uuid,
) -> Result<RealmDescription, WardenClientError> {
    match communicate(communicator, WardenCommand::InspectRealm { uuid }).await? {
        WardenResponse::InspectedRealm { description } => Ok(description),
        response => Err(handle_error_response(response)),
    }
}

pub async fn list_realms(
    communicator: &mut Communicator,
) -> Result<Vec<RealmDescription>, WardenClientError> {
    match communicate(communicator, WardenCommand::ListRealms).await? {
        WardenResponse::ListedRealms { realms_description } => Ok(realms_description),
        response => Err(handle_error_response(response)),
    }
}

pub async fn create_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    config: ApplicationConfig,
) -> Result<Uuid, WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::CreateApplication { realm_uuid, config },
    )
    .await?
    {
        WardenResponse::CreatedApplication { uuid } => Ok(uuid),
        response => Err(handle_error_response(response)),
    }
}

pub async fn update_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    application_uuid: Uuid,
    config: ApplicationConfig,
) -> Result<(), WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::UpdateApplication {
            realm_uuid,
            application_uuid,
            config,
        },
    )
    .await?
    {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn start_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    application_uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::StartApplication {
            realm_uuid,
            application_uuid,
        },
    )
    .await?
    {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn stop_application(
    communicator: &mut Communicator,
    realm_uuid: Uuid,
    application_uuid: Uuid,
) -> Result<(), WardenClientError> {
    match communicate(
        communicator,
        WardenCommand::StopApplication {
            realm_uuid,
            application_uuid,
        },
    )
    .await?
    {
        WardenResponse::Ok => Ok(()),
        response => Err(handle_error_response(response)),
    }
}

pub async fn connect_to_warden_socket(
    warden_socket_path: PathBuf,
) -> Result<UnixStream, WardenClientError> {
    UnixStream::connect(&warden_socket_path)
        .await
        .map_err(|err| WardenClientError::ConnectionFailed {
            socket_path: warden_socket_path,
            details: err,
        })
}

async fn communicate(
    communicator: &mut Communicator,
    command: WardenCommand,
) -> Result<WardenResponse, WardenClientError> {
    communicator
        .send(command)
        .await
        .map_err(WardenClientError::CommunicationFail)?;
    communicator
        .recv()
        .await
        .map_err(WardenClientError::CommunicationFail)
}

fn handle_error_response(response: WardenResponse) -> WardenClientError {
    match response {
        WardenResponse::Error { warden_error } => {
            WardenClientError::WardenOperationFail(warden_error)
        }
        response => WardenClientError::InvalidResponse { response },
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, str::FromStr, sync::Arc};

    use tokio::{
        net::{UnixListener, UnixStream},
        task::JoinHandle,
    };
    use tokio_util::sync::CancellationToken;
    use utils::serde::json_framed::JsonFramed;
    use uuid::Uuid;
    use warden_client::{
        application::ApplicationConfig,
        realm::{
            CpuConfig, KernelConfig, MemoryConfig, NetworkConfig, RealmConfig, RealmDescription,
            State,
        },
        warden::{WardenCommand, WardenDaemonError, WardenResponse},
    };

    use crate::{request_handlers::handle_error_response, warden_client_error::WardenClientError};

    use super::Communicator;

    type Respondent = JsonFramed<UnixStream, WardenCommand, WardenResponse>;

    #[test]
    fn handle_warden_error_response() {
        let warden_response = WardenResponse::Error {
            warden_error: WardenDaemonError::UnknownCommand,
        };
        let result = match handle_error_response(warden_response) {
            WardenClientError::WardenOperationFail(_) => Ok(()),
            _ => Err(()),
        };
        assert!(result.is_ok());
    }

    #[test]
    fn handle_other_warden_response() {
        let warden_response = WardenResponse::Ok;
        let result = match handle_error_response(warden_response) {
            WardenClientError::InvalidResponse { response: _ } => Ok(()),
            _ => Err(()),
        };
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn stop_application() {
        let realm_uuid = Uuid::new_v4();
        let application_uuid = Uuid::new_v4();
        let command = WardenCommand::StopApplication {
            realm_uuid,
            application_uuid,
        };
        const RESPONSE: WardenResponse = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(RESPONSE, respondent).await;

        assert!(
            super::stop_application(&mut communicator, realm_uuid, application_uuid)
                .await
                .is_ok()
        );
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn start_application() {
        let realm_uuid = Uuid::new_v4();
        let application_uuid = Uuid::new_v4();
        let command = WardenCommand::StartApplication {
            realm_uuid,
            application_uuid,
        };
        const RESPONSE: WardenResponse = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(RESPONSE, respondent).await;

        assert!(
            super::start_application(&mut communicator, realm_uuid, application_uuid)
                .await
                .is_ok()
        );
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn update_application() {
        let realm_uuid = Uuid::new_v4();
        let application_uuid = Uuid::new_v4();
        let command = WardenCommand::UpdateApplication {
            realm_uuid,
            application_uuid,
            config: create_example_client_app_config(),
        };
        const RESPONSE: WardenResponse = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(RESPONSE, respondent).await;

        assert!(super::update_application(
            &mut communicator,
            realm_uuid,
            application_uuid,
            create_example_client_app_config()
        )
        .await
        .is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn create_application() {
        let realm_uuid = Uuid::new_v4();
        let application_uuid = Uuid::new_v4();
        let command = WardenCommand::CreateApplication {
            realm_uuid,
            config: create_example_client_app_config(),
        };
        let response = WardenResponse::CreatedApplication {
            uuid: application_uuid,
        };
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(
            super::create_application(&mut communicator, realm_uuid, create_example_client_app_config())
                .await
                .is_ok()
        );
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn create_realm() {
        let realm_uuid = Uuid::new_v4();
        let command = WardenCommand::CreateRealm {
            config: create_example_realm_config(),
        };
        let response = WardenResponse::CreatedRealm { uuid: realm_uuid };
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(
            super::create_realm(&mut communicator, create_example_realm_config())
                .await
                .is_ok()
        );
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn start_realm() {
        let realm_uuid = Uuid::new_v4();
        let command = WardenCommand::StartRealm { uuid: realm_uuid };
        let response = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(super::start_realm(&mut communicator, realm_uuid)
            .await
            .is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn stop_realm() {
        let realm_uuid = Uuid::new_v4();
        let command = WardenCommand::StopRealm { uuid: realm_uuid };
        let response = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(super::stop_realm(&mut communicator, realm_uuid)
            .await
            .is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn destroy_realm() {
        let realm_uuid = Uuid::new_v4();
        let command = WardenCommand::DestroyRealm { uuid: realm_uuid };
        let response = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(super::destroy_realm(&mut communicator, realm_uuid)
            .await
            .is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn reboot_realm() {
        let realm_uuid = Uuid::new_v4();
        let command = WardenCommand::RebootRealm { uuid: realm_uuid };
        let response = WardenResponse::Ok;
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(super::reboot_realm(&mut communicator, realm_uuid)
            .await
            .is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn inspect_realm() {
        let realm_uuid = Uuid::new_v4();
        let command = WardenCommand::InspectRealm { uuid: realm_uuid };
        let response = WardenResponse::InspectedRealm {
            description: RealmDescription {
                uuid: realm_uuid,
                state: State::Halted,
                applications: vec![]
            },
        };
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(super::inspect_realm(&mut communicator, realm_uuid)
            .await
            .is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn list_realms() {
        let command = WardenCommand::ListRealms {};
        let response = WardenResponse::ListedRealms {
            realms_description: vec![],
        };
        let (mut communicator, respondent) = create_communicators();

        let result = receive_message_and_send_response(response, respondent).await;

        assert!(super::list_realms(&mut communicator).await.is_ok());
        assert_eq!(result.await.unwrap(), command);
    }

    #[tokio::test]
    async fn connect_to_warden_socket() {
        let socket_path = PathBuf::from_str(&format!("/tmp/{}", Uuid::new_v4())).unwrap();
        let waiting_token = Arc::new(CancellationToken::new());
        let warden_socket_path = socket_path.clone();
        let synchronization_token = waiting_token.clone();
        let server =
            tokio::spawn(async { UnixServer::new(socket_path, synchronization_token).await });
        waiting_token.cancelled().await;
        assert!(super::connect_to_warden_socket(warden_socket_path)
            .await
            .is_ok());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn connect_to_offline_warden_socket() {
        let socket_path = PathBuf::from_str(&format!("/tmp/{}", Uuid::new_v4())).unwrap();
        let result = match super::connect_to_warden_socket(socket_path)
            .await
            .err()
            .unwrap()
        {
            WardenClientError::ConnectionFailed {
                socket_path: _,
                details: _,
            } => Ok(()),
            _ => Err(()),
        };
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn communicate() {
        const COMMAND: WardenCommand = WardenCommand::ListRealms;
        const RESPONSE: WardenResponse = WardenResponse::Ok;
        let (mut communicator, mut respondent) = create_communicators();

        let result = tokio::spawn(async move {
            let received_command = respondent.recv().await.unwrap();
            respondent.send(RESPONSE).await.unwrap();
            received_command
        });
        communicator.send(COMMAND).await.unwrap();
        assert_eq!(communicator.recv().await.unwrap(), RESPONSE);
        assert_eq!(result.await.unwrap(), COMMAND);
    }

    fn create_communicators() -> (Communicator, Respondent) {
        let (communicator_stream, respondent_stream) = UnixStream::pair().unwrap();
        (
            Communicator::new(communicator_stream),
            Respondent::new(respondent_stream),
        )
    }

    async fn receive_message_and_send_response(
        response: WardenResponse,
        mut respondent: Respondent,
    ) -> JoinHandle<WardenCommand> {
        tokio::spawn(async move {
            let received_command = respondent.recv().await.unwrap();
            respondent.send(response).await.unwrap();
            received_command
        })
    }

    struct UnixServer {
        socket_path: PathBuf,
        _unix_listener: UnixListener,
        _unix_stream: UnixStream,
    }

    impl UnixServer {
        pub async fn new(
            socket_path: PathBuf,
            synchronization_token: Arc<CancellationToken>,
        ) -> Self {
            let unix_listener = UnixListener::bind(&socket_path).unwrap();
            let (unix_stream, _) = {
                let unix_listener = unix_listener.accept();
                synchronization_token.cancel();
                unix_listener.await.unwrap()
            };
            Self {
                socket_path,
                _unix_listener: unix_listener,
                _unix_stream: unix_stream,
            }
        }
    }

    impl Drop for UnixServer {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }

    pub fn create_example_realm_config() -> RealmConfig {
        RealmConfig {
            machine: String::new(),
            cpu: CpuConfig {
                cpu: String::new(),
                cores_number: 0,
            },
            memory: MemoryConfig { ram_size: 0 },
            network: NetworkConfig {
                vsock_cid: 0,
                tap_device: String::new(),
                mac_address: String::new(),
                hardware_device: None,
                remote_terminal_uri: None,
            },
            kernel: KernelConfig {
                kernel_path: PathBuf::new(),
            },
        }
    }

    fn create_example_client_app_config() -> ApplicationConfig {
        ApplicationConfig { name: String::new(), version: String::new(), image_registry: String::new(), image_storage_size_mb: 0, data_storage_size_mb: 0 }
    }
}
