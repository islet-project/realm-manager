use std::{
    ffi::OsStr,
    io,
    process::{CommandArgs, ExitStatus, Stdio},
    sync::Arc,
};

use log::{error, trace};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    select,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum VmHandlerError {
    #[error("Unable to spawn Vm: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Unable to launch Vm: {0}")]
    Launch(ExitStatus),
    #[error("Unable to kill Vm: {0}")]
    Kill(#[source] std::io::Error),
    #[error("Unable to get realm's exit code: {0}")]
    Wait(#[source] std::io::Error),
    #[error("Unable to read realm's output: {0}")]
    Read(#[source] std::io::Error),
}

pub struct VmHandler {
    vm_process: Arc<Mutex<Child>>,
    cancellation_token: Arc<CancellationToken>,
    communication_thread_handle: JoinHandle<()>,
}

impl VmHandler {
    pub async fn new(
        program: &OsStr,
        args: CommandArgs<'_>,
        vm_id: Uuid,
    ) -> Result<VmHandler, VmHandlerError> {
        let mut command = Command::new(program);
        command.args(args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let vm_process = Arc::new(Mutex::new(command.spawn().map_err(VmHandlerError::Spawn)?));
        let status = vm_process
            .lock()
            .await
            .try_wait()
            .map_err(VmHandlerError::Wait)?;
        match status {
            Some(exit_status) => Err(VmHandlerError::Launch(exit_status)),
            None => Ok({
                let cancellation_token = Arc::new(CancellationToken::new());
                let communication_thread_handle =
                    Self::spawn_log_thread(vm_process.clone(), cancellation_token.clone(), vm_id);
                VmHandler {
                    vm_process,
                    cancellation_token,
                    communication_thread_handle,
                }
            }),
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), VmHandlerError> {
        self.vm_process
            .lock()
            .await
            .kill()
            .await
            .map_err(VmHandlerError::Kill)?;
        self.cancellation_token.cancel();
        self.communication_thread_handle.abort();
        self.vm_process
            .lock()
            .await
            .wait()
            .await
            .map(|_| ())
            .map_err(VmHandlerError::Wait)
    }

    pub async fn try_get_exit_status(&mut self) -> Result<Option<ExitStatus>, io::Error> {
        self.vm_process.lock().await.try_wait()
    }

    fn spawn_log_thread(
        process: Arc<Mutex<Child>>,
        cancellation_token: Arc<CancellationToken>,
        uuid: Uuid,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            Self::gather_output(process, cancellation_token, uuid).await;
        })
    }

    async fn read_line(mut source: impl AsyncBufReadExt + Unpin) -> Result<String, VmHandlerError> {
        let mut line = String::new();
        let _ = source
            .read_line(&mut line)
            .await
            .map_err(VmHandlerError::Read)?;
        Ok(line)
    }

    async fn gather_output(
        process: Arc<Mutex<Child>>,
        cancellation_token: Arc<CancellationToken>,
        uuid: Uuid,
    ) {
        let (mut std_out, mut std_out_open) = {
            if let Some(std_out) = process.lock().await.stdout.take() {
                (BufReader::new(std_out), true)
            } else {
                error!("Unable to read std_out from realm with id: {}", uuid);
                return;
            }
        };
        let (mut std_err, mut std_err_open) = {
            if let Some(std_err) = process.lock().await.stderr.take() {
                (BufReader::new(std_err), true)
            } else {
                error!("Unable to read std_err from realm with id: {}", uuid);
                return;
            }
        };
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    return ;
                },
                std_out_log = Self::read_line(&mut std_out), if std_out_open => {
                    if let Ok(message) = std_out_log {
                        if message.is_empty() {
                            std_out_open = false;
                        } else {
                            trace!("Realm: {}: {}", uuid, message);
                        }
                    }
                },
                std_err_log = Self::read_line(&mut std_err), if std_err_open => {
                    if let Ok(message) = std_err_log {
                        if message.is_empty() {
                            std_err_open = false;
                        } else {
                            trace!("Realm: {} std_err: {}", uuid, message);
                        }
                    }
                }
            }
        }
    }
}
