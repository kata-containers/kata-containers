// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use common::{
    message::Message,
    types::{Request, Response},
    RuntimeHandler, RuntimeInstance,
};
use kata_types::{annotations::Annotation, config::TomlConfig};
use tokio::sync::{mpsc::Sender, RwLock};

#[cfg(feature = "linux")]
use linux_container::LinuxContainer;
#[cfg(feature = "virt")]
use virt_container::VirtContainer;
#[cfg(feature = "wasm")]
use wasm_container::WasmContainer;

struct RuntimeHandlerManagerInner {
    id: String,
    msg_sender: Sender<Message>,
    runtime_instance: Option<Arc<RuntimeInstance>>,
}

impl RuntimeHandlerManagerInner {
    fn new(id: &str, msg_sender: Sender<Message>) -> Result<Self> {
        Ok(Self {
            id: id.to_string(),
            msg_sender,
            runtime_instance: None,
        })
    }

    async fn init_runtime_handler(
        &mut self,
        netns: Option<String>,
        config: &TomlConfig,
    ) -> Result<()> {
        info!(sl!(), "new runtime handler {}", &config.runtime.name);
        let runtime_handler = match config.runtime.name.as_str() {
            #[cfg(feature = "linux")]
            name if name == LinuxContainer::name() => LinuxContainer::new_handler(),
            #[cfg(feature = "wasm")]
            name if name == WasmContainer::name() => WasmContainer::new_handler(),
            #[cfg(feature = "virt")]
            name if name == VirtContainer::name() || name.is_empty() => {
                VirtContainer::new_handler()
            }
            _ => return Err(anyhow!("Unsupported runtime: {}", &config.runtime.name)),
        };
        let runtime_instance = runtime_handler
            .new_instance(&self.id, self.msg_sender.clone(), config)
            .await
            .context("new runtime instance")?;

        // start sandbox
        runtime_instance
            .sandbox
            .start(netns, config)
            .await
            .context("start sandbox")?;
        self.runtime_instance = Some(Arc::new(runtime_instance));
        Ok(())
    }

    async fn try_init(&mut self, spec: &oci::Spec) -> Result<()> {
        // return if runtime instance has init
        if self.runtime_instance.is_some() {
            return Ok(());
        }

        #[cfg(feature = "linux")]
        LinuxContainer::init().context("init linux container")?;
        #[cfg(feature = "wasm")]
        WasmContainer::init().context("init wasm container")?;
        #[cfg(feature = "virt")]
        VirtContainer::init().context("init virt container")?;

        let netns = if let Some(linux) = &spec.linux {
            let mut netns = None;
            for ns in &linux.namespaces {
                if ns.r#type.as_str() != oci::NETWORKNAMESPACE {
                    continue;
                }

                if !ns.path.is_empty() {
                    netns = Some(ns.path.clone());
                    break;
                }
            }
            netns
        } else {
            None
        };

        let config = load_config(spec).context("load config")?;
        self.init_runtime_handler(netns, &config)
            .await
            .context("init runtime handler")?;

        Ok(())
    }

    fn get_runtime_instance(&self) -> Option<Arc<RuntimeInstance>> {
        self.runtime_instance.clone()
    }
}

unsafe impl Send for RuntimeHandlerManager {}
unsafe impl Sync for RuntimeHandlerManager {}
pub struct RuntimeHandlerManager {
    inner: Arc<RwLock<RuntimeHandlerManagerInner>>,
}

impl RuntimeHandlerManager {
    pub async fn new(id: &str, msg_sender: Sender<Message>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(RuntimeHandlerManagerInner::new(
                id, msg_sender,
            )?)),
        })
    }

    pub fn cleanup(_id: &str) -> Result<()> {
        // TODO: load runtime from persist and cleanup
        Ok(())
    }

    async fn get_runtime_instance(&self) -> Result<Arc<RuntimeInstance>> {
        let inner = self.inner.read().await;
        inner
            .get_runtime_instance()
            .ok_or_else(|| anyhow!("runtime not ready"))
    }

    async fn try_init_runtime_instance(&self, spec: &oci::Spec) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.try_init(spec).await
    }

    pub async fn handler_message(&self, req: Request) -> Result<Response> {
        if let Request::CreateContainer(req) = req {
            // get oci spec
            let bundler_path = format!("{}/{}", req.bundle, oci::OCI_SPEC_CONFIG_FILE_NAME);
            let spec = oci::Spec::load(&bundler_path).context("load spec")?;

            self.try_init_runtime_instance(&spec)
                .await
                .context("try init runtime instance")?;
            let instance = self
                .get_runtime_instance()
                .await
                .context("get runtime instance")?;

            let shim_pid = instance
                .container_manager
                .create_container(req, spec)
                .await
                .context("create container")?;
            Ok(Response::CreateContainer(shim_pid))
        } else {
            self.handler_request(req).await.context("handler request")
        }
    }

    pub async fn handler_request(&self, req: Request) -> Result<Response> {
        let instance = self
            .get_runtime_instance()
            .await
            .context("get runtime instance")?;
        let sandbox = instance.sandbox.clone();
        let cm = instance.container_manager.clone();

        match req {
            Request::CreateContainer(req) => Err(anyhow!("Unreachable request {:?}", req)),
            Request::CloseProcessIO(process_id) => {
                cm.close_process_io(&process_id).await.context("close io")?;
                Ok(Response::CloseProcessIO)
            }
            Request::DeleteProcess(process_id) => {
                let resp = cm.delete_process(&process_id).await.context("do delete")?;
                Ok(Response::DeleteProcess(resp))
            }
            Request::ExecProcess(req) => {
                cm.exec_process(req).await.context("exec")?;
                Ok(Response::ExecProcess)
            }
            Request::KillProcess(req) => {
                cm.kill_process(&req).await.context("kill process")?;
                Ok(Response::KillProcess)
            }
            Request::ShutdownContainer(req) => {
                if cm.need_shutdown_sandbox(&req).await {
                    sandbox.shutdown().await.context("do shutdown")?;
                }
                Ok(Response::ShutdownContainer)
            }
            Request::WaitProcess(process_id) => {
                let exit_status = cm.wait_process(&process_id).await.context("wait process")?;
                if cm.is_sandbox_container(&process_id).await {
                    sandbox.stop().await.context("stop sandbox")?;
                }
                Ok(Response::WaitProcess(exit_status))
            }
            Request::StartProcess(process_id) => {
                let shim_pid = cm
                    .start_process(&process_id)
                    .await
                    .context("start process")?;
                Ok(Response::StartProcess(shim_pid))
            }

            Request::StateProcess(process_id) => {
                let state = cm
                    .state_process(&process_id)
                    .await
                    .context("state process")?;
                Ok(Response::StateProcess(state))
            }
            Request::PauseContainer(container_id) => {
                cm.pause_container(&container_id)
                    .await
                    .context("pause container")?;
                Ok(Response::PauseContainer)
            }
            Request::ResumeContainer(container_id) => {
                cm.resume_container(&container_id)
                    .await
                    .context("resume container")?;
                Ok(Response::ResumeContainer)
            }
            Request::ResizeProcessPTY(req) => {
                cm.resize_process_pty(&req).await.context("resize pty")?;
                Ok(Response::ResizeProcessPTY)
            }
            Request::StatsContainer(container_id) => {
                let stats = cm
                    .stats_container(&container_id)
                    .await
                    .context("stats container")?;
                Ok(Response::StatsContainer(stats))
            }
            Request::UpdateContainer(req) => {
                cm.update_container(req).await.context("update container")?;
                Ok(Response::UpdateContainer)
            }
            Request::Pid => Ok(Response::Pid(cm.pid().await.context("pid")?)),
            Request::ConnectContainer(container_id) => Ok(Response::ConnectContainer(
                cm.connect_container(&container_id)
                    .await
                    .context("connect")?,
            )),
        }
    }
}

/// Config override ordering(high to low):
/// 1. podsandbox annotation
/// 2. shimv2 create task option
/// TODO: https://github.com/kata-containers/kata-containers/issues/3961
/// 3. environment
fn load_config(spec: &oci::Spec) -> Result<TomlConfig> {
    const KATA_CONF_FILE: &str = "KATA_CONF_FILE";
    let annotation = Annotation::new(spec.annotations.clone());
    let config_path = if let Some(path) = annotation.get_sandbox_config_path() {
        path
    } else if let Ok(path) = std::env::var(KATA_CONF_FILE) {
        path
    } else {
        String::from("")
    };
    info!(sl!(), "get config path {:?}", &config_path);
    let (mut toml_config, _) =
        TomlConfig::load_from_file(&config_path).context("load toml config")?;
    annotation.update_config_by_annotation(&mut toml_config)?;
    info!(sl!(), "get config content {:?}", &toml_config);
    Ok(toml_config)
}
