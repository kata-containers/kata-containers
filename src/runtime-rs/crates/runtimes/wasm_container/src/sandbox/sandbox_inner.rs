use super::signal::signal_handler;
use crate::{container_manager::container::container::Container, SANDBOX_BASE};
use common::SandboxNetworkEnv;
use kata_agent::namespace::{Namespace, NSTYPEIPC, NSTYPEPID, NSTYPEUTS};

use anyhow::{anyhow, Context, Result};
use futures::future::join_all;
use oci::LinuxNamespace;
use slog::Logger;
use std::{collections::HashMap, mem::take, sync::Arc};
use tokio::{
    sync::watch::{channel, Receiver, Sender},
    sync::RwLock,
    task::JoinHandle,
};

const ERR_NO_LINUX_FIELD: &str = "Spec does not contain linux field";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SandboxState {
    Init,
    Running,
    Stopped,
}

#[derive(Debug)]
pub struct WasmSandboxInner {
    pub logger: Logger,
    pub base_path: String,
    pub hostname: String,
    pub shared_utsns: Namespace,
    pub shared_ipcns: Namespace,
    pub state: SandboxState,
    pub tasks: Vec<JoinHandle<Result<()>>>,
    pub shutdown_tx: Sender<bool>,
    pub shutdown_rx: Receiver<bool>,
}

impl WasmSandboxInner {
    pub fn new(logger: &Logger, sid: &str) -> Self {
        let (shutdown_tx, shutdown_rx) = channel(false);

        WasmSandboxInner {
            logger: logger.new(o!("subsystem" => "wasm_sandbox_inner")),
            base_path: [SANDBOX_BASE.as_str(), sid].join("/"),
            hostname: String::new(),
            shared_utsns: Namespace::new(&logger),
            shared_ipcns: Namespace::new(&logger),
            state: SandboxState::Init,
            tasks: vec![],
            shutdown_tx,
            shutdown_rx,
        }
    }

    pub async fn start(
        &mut self,
        _dns: Vec<String>,
        spec: &oci::Spec,
        _state: &oci::State,
        _network_env: SandboxNetworkEnv,
    ) -> Result<()> {
        if self.state == SandboxState::Running {
            warn!(sl!(), "wasm sandbox is running, no need to start");
            return Ok(());
        }

        self.hostname = spec.hostname.clone();
        self.state = SandboxState::Running;

        self.prepare_shared_namespaces()
            .await
            .map_err(|e| anyhow!(format!("failed to setup shared ns {:?}", e)))?;

        Ok(())
    }

    pub async fn install_signal_handler(
        &mut self,
        containers: Arc<RwLock<HashMap<String, Container>>>,
    ) -> Result<()> {
        let signal_handler_task =
            tokio::spawn(signal_handler(containers.clone(), self.shutdown_rx.clone()));

        self.tasks.push(signal_handler_task);

        Ok(())
    }

    async fn prepare_shared_namespaces(&mut self) -> Result<()> {
        // Set up shared IPC namespace
        self.shared_ipcns = Namespace::new(&self.logger)
            .get_ipc()
            .setup()
            .await
            .context("Failed to setup persistent IPC namespace")?;

        // Set up shared UTS namespace
        self.shared_utsns = Namespace::new(&self.logger)
            .get_uts(self.hostname.as_str())
            .setup()
            .await
            .context("Failed to setup persistent UTS namespace")?;

        Ok(())
    }

    pub async fn update_container_namespaces(&self, spec: &mut oci::Spec) -> Result<()> {
        let linux = spec
            .linux
            .as_mut()
            .ok_or_else(|| anyhow!(ERR_NO_LINUX_FIELD))?;

        let namespaces = linux.namespaces.as_mut_slice();
        for namespace in namespaces.iter_mut() {
            if namespace.r#type == NSTYPEIPC {
                namespace.path = self.shared_ipcns.path.clone();
                continue;
            }
            if namespace.r#type == NSTYPEUTS {
                namespace.path = self.shared_utsns.path.clone();
                continue;
            }
        }
        // update pid namespace
        let mut pid_ns = LinuxNamespace {
            r#type: NSTYPEPID.to_string(),
            ..Default::default()
        };

        // check pidns
        for n in linux.namespaces.iter() {
            if n.r#type.as_str() == oci::PIDNAMESPACE && !n.path.is_empty() {
                pid_ns.path = String::from(n.path.as_str());
                break;
            }
        }

        linux.namespaces.push(pid_ns);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.shutdown_tx
            .send(true)
            .map_err(|e| anyhow!(e).context("failed to request shutdown"))?;

        let tasks = take(&mut self.tasks);
        let results = join_all(tasks).await;
        for result in results {
            if let Err(e) = result {
                error!(self.logger, "wait task error: {:#?}", e);
                return Err(anyhow!(format!("wait task error: {:#?}", e)));
            }
        }

        self.state = SandboxState::Stopped;

        Ok(())
    }
}
