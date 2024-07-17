use std::{path::PathBuf, sync::Arc};

use log::{debug, error, info};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tokio::{select, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::command_handler::client_command_handler::Client;
use crate::managers::warden::Warden;

pub enum UnixSocketServerError {
    SocketBindingFail,
    ClientAcceptFail,
}

pub struct UnixSocketServer;

impl UnixSocketServer {
    pub async fn listen<T: Client>(
        host: Arc<Mutex<Box<dyn Warden + Send + Sync>>>,
        token: Arc<CancellationToken>,
        socket: PathBuf,
    ) -> Result<(), UnixSocketServerError> {
        info!("Starting Unix Socket Server!");
        let mut clients_set = JoinSet::new();

        let listener =
            UnixListener::bind(socket).map_err(|_| UnixSocketServerError::SocketBindingFail)?;

        loop {
            select! {
                accepted_connection = listener.accept() => {
                    let (stream, _addr) = accepted_connection.map_err(|_| UnixSocketServerError::ClientAcceptFail)?;
                    let host = host.clone();
                    let token = token.clone();
                    info!("Client connected to the server!");
                    let _ = clients_set.spawn(async move {
                        // TODO! Handle this output
                        if let Err(e) = T::handle_connection(host, stream, token).await {
                            error!("{e:?}");
                        }
                    });
                }
                exited_client = clients_set.join_next() => {
                    debug!("Client {:?} has exited", exited_client);
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
}
