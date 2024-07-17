use log::{debug, error, info};
use std::fs::remove_file;
use std::io;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::net::unix::SocketAddr;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use tokio::{select, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::command_handler::client_command_handler::Client;
use crate::managers::warden::Warden;

#[derive(Error, Debug)]
pub enum UnixSocketServerError {
    #[error("Socket operation failed: {0}")]
    SocketFail(#[from] io::Error),
}

pub struct UnixSocketServer;

impl UnixSocketServer {
    pub async fn listen<T: Client>(
        warden: Box<dyn Warden + Send + Sync>,
        token: Arc<CancellationToken>,
        socket: PathBuf,
    ) -> Result<(), UnixSocketServerError> {
        info!("Starting Unix Socket Server!");
        let mut clients_set = JoinSet::new();
        let listener = UnixSocketServer::create_listener(socket)?;
        let warden = Arc::new(Mutex::new(warden));
        loop {
            select! {
                accepted_connection = listener.accept() => {
                    info!("Client connected to the server!");
                    UnixSocketServer::handle_connection::<T>(accepted_connection.map_err(|err| UnixSocketServerError::SocketFail(err))?, &mut clients_set, warden.clone(), token.clone());
                }
                exited_client = clients_set.join_next(), if !clients_set.is_empty() => {
                    debug!("Client has exited with result: {:?}", exited_client);
                }
                _ = token.cancelled() => {
                    break;
                }
            }
        }

        while let Some(v) = clients_set.join_next().await {
            debug!("Client thread {:?} joined", v);
        }

        Ok(())
    }

    fn handle_connection<T: Client>(
        (stream, address): (UnixStream, SocketAddr),
        clients_set: &mut JoinSet<()>,
        warden: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        token: Arc<CancellationToken>,
    ) -> AbortHandle {
        clients_set.spawn(async move {
            match T::handle_connection(warden, stream, token).await {
                Err(err) => {
                    error!("{err:?}");
                }
                Ok(_) => {
                    debug!("Connection: {:?} ended impeccably!", address);
                }
            }
        })
    }

    fn create_listener(socket: PathBuf) -> Result<UnixListener, UnixSocketServerError> {
        if socket.exists() {
            remove_file(&socket).map_err(|err| UnixSocketServerError::SocketFail(err))?;
        }
        UnixListener::bind(socket).map_err(|err| UnixSocketServerError::SocketFail(err))
    }
}