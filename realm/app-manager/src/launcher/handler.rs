use std::{collections::HashMap, env::set_current_dir, os::unix::fs::chroot, path::PathBuf, process::{ExitStatus, Stdio}};

use async_trait::async_trait;
use log::info;
use nix::{errno::Errno, libc::{setgid, setuid}, sys::signal::{self, Signal}, unistd::Pid};
use thiserror::Error;
use tokio::{io::{AsyncBufRead, BufReader}, process::{Child, ChildStderr, ChildStdout, Command}, select, sync::mpsc::{self, Receiver, Sender}, task::{JoinError, JoinHandle}};
use tokio::io::AsyncBufReadExt;

use super::{ApplicationHandler, Result};

#[derive(Debug, Error)]
pub enum ApplicationHandlerError {
    #[error("Argv is empty")]
    EmptyArgv(),

    #[error("Application spawning error")]
    SpawnError(#[source] std::io::Error),

    #[error("Application is not running")]
    AppNotRunning(),

    #[error("Failed to send request to app handler")]
    RequestChannelError(#[source] mpsc::error::SendError<Request>),

    #[error("Failed to send response back from app handler")]
    ResponseChannelError(#[source] mpsc::error::SendError<Response>),

    #[error("Channel was closed")]
    ChannelClosed(),

    #[error("App handler invalid response")]
    AppHandlerInvalidResponse(),

    #[error("Failed to join the app handler thread")]
    AppThreadJoinError(#[from] JoinError),

    #[error("Error reading spawned process IO")]
    IOReadError(#[source] std::io::Error),

    #[error("Failed to send signal to the application")]
    FailedToSendSignal(#[source] Errno),

    #[error("Error while awaiting the spawned application")]
    WaitpidError(#[source] std::io::Error),

    #[error("Failed to kill the child after the parent is closing")]
    FailedToKillChild(#[source] std::io::Error)
}

pub struct ExecConfig {
    pub exec: PathBuf,
    pub argv: Vec<String>,
    pub envp: HashMap<String, String>,
    pub uid: u32,
    pub gid: u32,
    pub chroot: Option<PathBuf>,
    pub chdir: Option<PathBuf>,
}

#[derive(Debug)]
pub enum Request {
    Stop,
    Kill,
    Wait,
    TryWait
}
#[derive(Debug)]
pub enum Response {
    Exited(ExitStatus),
    MaybeExited(Option<ExitStatus>),
    Stopped()
}

pub struct SimpleApplicationHandler {
    config: ExecConfig,
    thread: Option<JoinHandle<Result<()>>>,
    channel: Option<(Sender<Request>, Receiver<Response>)>
}

impl SimpleApplicationHandler {
    pub fn new(config: ExecConfig) -> Self {
        Self {
            config,
            thread: None,
            channel: None
        }
    }

    async fn transaction(&mut self, req: Request) -> Result<Response> {
        let (tx, rx) = self.channel.as_mut()
            .ok_or(ApplicationHandlerError::AppNotRunning())?;

        tx.send(req)
            .await
            .map_err(ApplicationHandlerError::RequestChannelError)?;

        let resp = rx.recv()
            .await
            .ok_or(ApplicationHandlerError::ChannelClosed())?;

        Ok(resp)
    }

    async fn join_handler_thread(&mut self) -> Result<()> {
        let handle = self.thread.as_mut()
            .ok_or(ApplicationHandlerError::AppNotRunning())?;

        let result = handle.await.map_err(ApplicationHandlerError::AppThreadJoinError)?;

        self.thread = None;
        self.channel = None;

        result
    }
}

struct WardenThread {
    process: Child,
    stdout_reader: BufReader<ChildStdout>,
    stderr_reader: BufReader<ChildStderr>,
    stdout_open: bool,
    stderr_open: bool,
    tx: Sender<Response>,
    rx: Receiver<Request>
}

impl WardenThread {
    pub async fn start(mut process: Child, channel: (Sender<Response>, Receiver<Request>)) -> Result<()> {
        let stdout_reader = BufReader::new(process.stdout.take().unwrap());
        let stderr_reader = BufReader::new(process.stderr.take().unwrap());

        let mut warden = Self {
            process,
            stdout_reader,
            stderr_reader,
            stdout_open: true,
            stderr_open: true,
            tx: channel.0,
            rx: channel.1
        };

        warden.event_loop().await
    }

    async fn read_line(mut source: impl AsyncBufRead + Unpin) -> Result<String> {
        let mut line = String::new();
        let _ = source.read_line(&mut line)
            .await
            .map_err(ApplicationHandlerError::IOReadError)?;

        Ok(line)
    }

    async fn send_response(&mut self, response: Response) -> Result<()> {
        self.tx.send(response)
            .await
            .map_err(ApplicationHandlerError::ResponseChannelError)?;

        Ok(())
    }

    async fn after_exit(&mut self, req: Request) -> Result<()> {
        let status = self.process.wait()
            .await
            .map_err(ApplicationHandlerError::WaitpidError)?;

        let resp = match req {
            Request::Stop | Request::Kill => Response::Stopped(),
            Request::Wait => Response::Exited(status),
            Request::TryWait => Response::MaybeExited(Some(status))
        };
        self.send_response(resp).await?;

        Ok(())
    }

    async fn stop_and_respond(&mut self, req: Request) -> Result<()> {
        if let Some(pid) = self.process.id() {
            let pid = Pid::from_raw(pid as i32);

            match req {
                Request::Stop => signal::kill(pid, Signal::SIGTERM)
                    .map_err(ApplicationHandlerError::FailedToSendSignal)?,
                Request::Kill => signal::kill(pid, Signal::SIGKILL)
                    .map_err(ApplicationHandlerError::FailedToSendSignal)?,
                _ => {}
            };
        }

        self.after_exit(req).await?;

        Ok(())
    }

    async fn try_wait(&mut self) -> Result<()> {
        let status = self.process.try_wait()
            .map_err(ApplicationHandlerError::WaitpidError)?;
        let response = Response::MaybeExited(status);

        self.send_response(response).await
    }

    async fn handle_request(&mut self, req: Request) -> Result<()> {
        match req {
            Request::Stop | Request::Kill => self.stop_and_respond(req).await,
            Request::Wait => self.after_exit(req).await,
            Request::TryWait => self.try_wait().await
        }
    }

    async fn process_exited(&mut self) -> Result<()> {
        let request_opt = self.rx.recv().await;

        if let Some(request) = request_opt {
            self.after_exit(request).await?;
        }

        Ok(())
    }

    fn is_app_running(&self) -> bool {
        self.process.id().is_some()
    }

    async fn ensure_app_is_stopped(&mut self) {
        if self.is_app_running() {
            let _ = self.process.kill().await;
            let _ = self.process.wait().await;
        }
    }

    async fn event_loop(&mut self) -> Result<()> {
        loop {
            select! {
                request_opt = self.rx.recv() => {
                    if let Some(request) = request_opt {
                        self.handle_request(request).await?;
                    } else {
                        break;
                    }

                    if !self.is_app_running() {
                        break;
                    }
                }

                result = Self::read_line(&mut self.stdout_reader), if self.stdout_open => {
                    let line = result?;

                    if line.is_empty() {
                        self.stdout_open = false;
                    }

                    info!("Application stdout: {}", line);
                }

                result = Self::read_line(&mut self.stderr_reader), if self.stderr_open => {
                    let line = result?;

                    if line.is_empty() {
                        self.stderr_open = false;
                    }

                    info!("Application stderr: {}", line);
                }

                _ = self.process.wait() => {
                    self.process_exited().await?;
                    break;
                }
            }
        }

        self.ensure_app_is_stopped().await;
        info!("App handling thread exited");

        Ok(())
    }
}

#[async_trait]
impl ApplicationHandler for SimpleApplicationHandler {
    async fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.config.exec);

        if self.config.argv.is_empty() {
            return Err(ApplicationHandlerError::EmptyArgv().into());
        }

        cmd.arg0(&self.config.argv[0]);
        cmd.args(self.config.argv.iter().skip(1));
        cmd.env_clear();
        cmd.envs(self.config.envp.iter());

        let chrootdir = self.config.chroot.clone();
        let cwd = self.config.chdir.clone();
        let uid = self.config.uid;
        let gid = self.config.gid;

        unsafe {
            cmd.pre_exec(move || {
                if let Some(dir) = chrootdir.as_ref() {
                    chroot(dir)?;
                    set_current_dir("/")?;
                }

                if let Some(dir) = cwd.as_ref() {
                    set_current_dir(dir)?;
                }

                setuid(uid);
                setgid(gid);

                Ok(())
            });
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let process = cmd.spawn()
            .map_err(ApplicationHandlerError::SpawnError)?;

        let (tx1, rx1) = mpsc::channel::<Request>(1);
        let (tx2, rx2) = mpsc::channel::<Response>(1);

        self.channel = Some((tx1, rx2));
        self.thread = Some(tokio::spawn(async move {
            WardenThread::start(process, (tx2, rx1)).await
        }));

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        let resp = self.transaction(Request::Stop).await?;
        self.join_handler_thread().await?;

        match resp {
            Response::Stopped() => Ok(()),
            _ => Err(ApplicationHandlerError::AppHandlerInvalidResponse().into())
        }
    }

    async fn kill(&mut self) -> Result<()> {
        let resp = self.transaction(Request::Kill).await?;
        self.join_handler_thread().await?;

        match resp {
            Response::Stopped() => Ok(()),
            _ => Err(ApplicationHandlerError::AppHandlerInvalidResponse().into())
        }
    }

    async fn wait(&mut self) -> Result<ExitStatus> {
        let resp = self.transaction(Request::Wait).await?;
        self.join_handler_thread().await?;

        match resp {
            Response::Exited(status) => Ok(status),
            _ => Err(ApplicationHandlerError::AppHandlerInvalidResponse().into())
        }
    }

    async fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        let resp = self.transaction(Request::TryWait).await?;

        match resp {
            Response::MaybeExited(Some(status)) => {
                self.join_handler_thread().await?;
                Ok(Some(status))
            },
            Response::MaybeExited(None) => Ok(None),
            _ => Err(ApplicationHandlerError::AppHandlerInvalidResponse().into())
        }
    }
}
