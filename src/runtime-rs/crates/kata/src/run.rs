// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use ttrpc::server::Server;

use shim_proto::shim_ttrpc;
use virtcontainers::{Sandbox, TomlConfig};

use crate::config::load_configuration;
use crate::task_service::TaskService;
use crate::{Error, Result, ShimExecutor, KATA_BIND_FD};

impl ShimExecutor {
    // Implement ttrpc call from containerd
    // This functions should be the most outside one for "normal run" command action
    // So it does not return errors but directly write errors to stderr
    pub fn run(&mut self) {
        if let Err(e) = self.args.validate(false) {
            eprintln!("run shim err: {}", e);
            return;
        }

        if let Err(e) = self.long_run() {
            eprintln!("run shim error: {}", e);
            std::process::exit(1);
        }
    }

    fn long_run(&mut self) -> Result<()> {
        let _logger_guard = self.set_logger()?;
        let bundle_path = self.get_bundle_path()?;
        let toml_config = load_configuration(&bundle_path).map_err(Error::Config)?;
        let server_fd = get_server_fd()?;
        let sandbox = self.start_sandbox(toml_config, &bundle_path)?;
        let sandbox = Arc::new(Mutex::new(sandbox));
        let (tx, rx) = channel::<ServerMessage>();

        let server = self.start_ttrpc_server(server_fd, sandbox, tx)?;

        Self::handle_messages(server, rx)
    }

    fn set_logger(&mut self) -> Result<slog_async::AsyncGuard> {
        let bundle_path = self.get_bundle_path()?;
        let path = bundle_path.join("log");
        let fifo = std::fs::OpenOptions::new()
            .custom_flags(libc::O_NONBLOCK)
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
            .map_err(|e| Error::OpenFile(e, path))?;

        let level = if self.args.debug {
            slog::Level::Debug
        } else {
            slog::Level::Info
        };

        let (logger, async_guard) = logging::create_logger("kata-runtime-rs", "rund", level, fifo);

        // set global logger for slog
        // not reset global logger when drop
        slog_scope::set_global_logger(logger).cancel_reset();

        let level = if self.args.debug {
            log::Level::Debug
        } else {
            log::Level::Info
        };

        // set global logger for log
        // in case some dependent libraries use log instead of slog
        let _ = slog_stdlog::init_with_level(level).map_err(|_| Error::Logger)?;

        Ok(async_guard)
    }

    fn start_sandbox(&mut self, toml_config: TomlConfig, bundle_path: &Path) -> Result<Sandbox> {
        let mut sandbox =
            Sandbox::new(&self.args.id, toml_config, bundle_path).map_err(Error::Sandbox)?;
        sandbox.start().map_err(Error::Sandbox)?;

        Ok(sandbox)
    }

    fn start_ttrpc_server(
        &mut self,
        server_fd: RawFd,
        sandbox: Arc<Mutex<Sandbox>>,
        tx: Sender<ServerMessage>,
    ) -> Result<Server> {
        let mut server = unsafe { Server::from_raw_fd(server_fd) };
        let service = TaskService {
            pid: std::process::id(),
            sandbox,
            shutdown_sender: Mutex::new(tx),
        };
        let b = Box::new(service) as Box<dyn shim_ttrpc::Task + Send + Sync>;
        let b = Arc::new(b);
        let task_service = shim_ttrpc::create_task(b);

        server = server.register_service(task_service);
        server.start().map_err(Error::StartServer)?;

        Ok(server)
    }

    fn handle_messages(mut server: Server, rx: Receiver<ServerMessage>) -> Result<()> {
        for r in rx.iter() {
            match r.action {
                ServerAction::StartListen => {
                    if let Err(e) = server.start() {
                        slog::error!(slog_scope::logger(), "failed to start listen: {:?}", e);
                    }
                }
                ServerAction::StopListen => {
                    server = server.stop_listen();
                }
                ServerAction::ShutdownForce => {
                    server.stop_listen();
                    break;
                }
            }

            if let Some(ref callback) = r.callback {
                callback.send(()).map_err(|e| {
                    let s = format!("callback error {} when {:?}", e, r.action);
                    Error::WaitServer(s)
                })?;
            }
        }

        Ok(())
    }
}

fn get_server_fd() -> Result<RawFd> {
    let env_fd = std::env::var(KATA_BIND_FD).map_err(Error::EnvVar)?;
    let fd = env_fd.parse::<i32>().map_err(|_| Error::ServerFd(env_fd))?;
    Ok(fd)
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum ServerAction {
    StartListen,
    StopListen,
    ShutdownForce,
}

pub(crate) struct ServerMessage {
    pub action: ServerAction,
    pub callback: Option<Sender<()>>,
}

impl ServerMessage {
    pub fn new(action: ServerAction) -> Self {
        ServerMessage {
            action,
            callback: None,
        }
    }
}
