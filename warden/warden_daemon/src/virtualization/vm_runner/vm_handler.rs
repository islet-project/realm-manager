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
    process::{Child, ChildStderr, ChildStdout, Command},
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
    #[error("Unable to take stdout from realm: {0}")]
    StdOutTake(Uuid),
    #[error("Unable to take stderr from realm: {0}")]
    StdErrTake(Uuid),
}

pub struct VmHandler {
    vm_process: Arc<Mutex<Child>>,
    communication_thread_handle: JoinHandle<()>,
}

impl VmHandler {
    pub async fn new(
        program: &OsStr,
        args: CommandArgs<'_>,
        vm_id: Uuid,
    ) -> Result<VmHandler, VmHandlerError> {
        let command = Self::prepare_command(program, args);
        let (vm_process, status) = Self::spawn_vm_process(command)?;
        let (std_out, std_err) = Self::create_output_readers(&vm_process, vm_id).await?;

        match status {
            Some(exit_status) => Err(VmHandlerError::Launch(exit_status)),
            None => Ok({
                let communication_thread_handle = Self::spawn_log_thread(std_out, std_err, vm_id);
                VmHandler {
                    vm_process,
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

    async fn create_output_readers(
        vm_process: &Arc<Mutex<Child>>,
        vm_id: Uuid,
    ) -> Result<(BufReader<ChildStdout>, BufReader<ChildStderr>), VmHandlerError> {
        let std_out = BufReader::new(
            vm_process
                .lock()
                .await
                .stdout
                .take()
                .ok_or(VmHandlerError::StdOutTake(vm_id))?,
        );
        let std_err = BufReader::new(
            vm_process
                .lock()
                .await
                .stderr
                .take()
                .ok_or(VmHandlerError::StdErrTake(vm_id))?,
        );
        Ok((std_out, std_err))
    }

    fn prepare_command(program: &OsStr, args: CommandArgs<'_>) -> Command {
        let mut command = Command::new(program);
        command.args(args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command
    }

    fn spawn_vm_process(
        mut command: Command,
    ) -> Result<(Arc<Mutex<Child>>, Option<ExitStatus>), VmHandlerError> {
        let mut vm_process = command.spawn().map_err(VmHandlerError::Spawn)?;
        let status = vm_process.try_wait().map_err(VmHandlerError::Wait)?;
        Ok((Arc::new(Mutex::new(vm_process)), status))
    }

    fn spawn_log_thread(
        std_out: BufReader<ChildStdout>,
        std_err: BufReader<ChildStderr>,
        uuid: Uuid,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            Self::gather_output(std_out, std_err, uuid).await;
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
        mut std_out: BufReader<ChildStdout>,
        mut std_err: BufReader<ChildStderr>,
        uuid: Uuid,
    ) {
        let cancellation_token = CancellationToken::new();
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    return ;
                },
                std_out_log = Self::read_line(&mut std_out) => {
                    Self::handle_vm_output(std_out_log, &cancellation_token, uuid);
                },
                std_err_log = Self::read_line(&mut std_err) => {
                    Self::handle_vm_output(std_err_log, &cancellation_token, uuid);
                }
            }
        }
    }

    fn handle_vm_output(
        output: Result<String, VmHandlerError>,
        cancellation_token: &CancellationToken,
        uuid: Uuid,
    ) {
        if let Ok(message) = output {
            if message.is_empty() {
                cancellation_token.cancel()
            } else {
                trace!("Realm: {}: {}", uuid, message);
            }
        }
    }
}
