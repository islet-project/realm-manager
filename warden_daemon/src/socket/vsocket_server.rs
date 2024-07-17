use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use log::info;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::select;
use tokio::sync::oneshot::{Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;
use tokio_vsock::{VsockAddr, VsockListener, VsockStream};

use crate::managers::realm_client::{RealmCommand, RealmConnector, RealmSender, RealmSenderError};

pub struct VSockServerConfig {
    pub cid: u32,
    pub port: u32,
}

#[derive(Debug, Error)]
pub enum VSockServerError {
    #[error("")]
    AcceptError,
    #[error("")]
    AwakenError,
    #[error("")]
    UnexpectedConnection,
}

pub struct VSockServer {
    pub config: VSockServerConfig,
    cancel_token: Arc<CancellationToken>,
    waiting: Mutex<HashMap<u32, Sender<Box<dyn RealmSender + Send + Sync>>>>,
}

impl Drop for VSockServer {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

impl VSockServer {
    pub fn new(config: VSockServerConfig, cancel_token: Arc<CancellationToken>) -> Self {
        VSockServer {
            config,
            cancel_token,
            waiting: Mutex::new(HashMap::new()),
        }
    }

    pub async fn listen(
        handler: Arc<Mutex<VSockServer>>,
        token: Arc<CancellationToken>,
    ) -> Result<(), VSockServerError> {
        let config = &handler.as_ref().lock().await.config;
        let mut listener = VsockListener::bind(VsockAddr::new(config.cid, config.port)).unwrap();
        loop {
            select! {
                a_result = listener.accept() => {
                    match a_result {
                        Ok(result) => {
                            let mut handler = handler.lock().await;
                            handler.handle_accept(result).await?
                        },
                        Err(_) => return Err(VSockServerError::AcceptError)
                    }
                }
                _ = token.cancelled() => {return Ok(());}

            }
        }
    }

    async fn handle_accept(
        &mut self,
        accept_result: (VsockStream, VsockAddr),
    ) -> Result<(), VSockServerError> {
        let (stream, addr) = accept_result;
        let mut waiting = self.waiting.lock().await;
        if let Some(tx) = waiting.remove(&addr.cid()) {
            info!("Client has connected succesfully!");
            return tx
                .send(Box::new(VSockClient { stream }))
                .map_err(|_| VSockServerError::AwakenError);
        }
        Err(VSockServerError::UnexpectedConnection)
    }
}

struct VSockClient {
    stream: VsockStream,
}

#[async_trait]
impl RealmSender for VSockClient {
    async fn send(&mut self, data: RealmCommand) -> Result<(), RealmSenderError> {
        let serialized_data =
            serde_json::to_vec(&data).map_err(|_| RealmSenderError::CommandParsingFail(data))?;
        self.stream
            .write_all(&serialized_data)
            .await
            .map_err(|err| RealmSenderError::SendFail(err))
            .map(|_| ())
    }
}

#[async_trait]
impl RealmConnector for VSockServer {
    async fn acquire_realm_sender(
        &mut self,
        cid: u32,
    ) -> Receiver<Box<dyn RealmSender + Send + Sync>> {
        let mut waiting = self.waiting.lock().await;
        let (tx, rx) = oneshot::channel();
        waiting.insert(cid, tx);
        rx
    }
}
