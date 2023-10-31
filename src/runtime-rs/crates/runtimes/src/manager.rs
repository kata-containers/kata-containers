// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, path::PathBuf, str::from_utf8, sync::Arc};

use anyhow::{anyhow, Context, Result};
use common::{
    message::Message,
    types::{
        PlatformInfo, SandboxConfig, SandboxRequest, SandboxResponse, SandboxStatusInfo,
        StartSandboxInfo, TaskRequest, TaskResponse,
    },
    RuntimeHandler, RuntimeInstance, Sandbox, SandboxNetworkEnv,
};
use hypervisor::Param;
use kata_sys_util::spec::load_oci_spec;
use kata_types::{
    annotations::Annotation, config::default::DEFAULT_GUEST_DNS_FILE, config::TomlConfig,
};
#[cfg(feature = "linux")]
use linux_container::LinuxContainer;
use netns_rs::NetNs;
use persist::sandbox_persist::Persist;
use resource::{cpu_mem::initial_size::InitialSizeManager, network::generate_netns_name};
use shim_interface::shim_mgmt::ERR_NO_SHIM_SERVER;
use std::time::SystemTime;
use tokio::fs;
use tokio::sync::{mpsc::Sender, Mutex, RwLock};
use tracing::instrument;
#[cfg(feature = "virt")]
use virt_container::{
    sandbox::{SandboxRestoreArgs, VirtSandbox},
    sandbox_persist::SandboxState,
    VirtContainer,
};
#[cfg(feature = "wasm")]
use wasm_container::WasmContainer;

use crate::{
    shim_mgmt::server::MgmtServer,
    tracer::{KataTracer, ROOTSPAN},
};

struct RuntimeHandlerManagerInner {
    id: String,
    msg_sender: Sender<Message>,
    kata_tracer: Arc<Mutex<KataTracer>>,
    runtime_instance: Option<Arc<RuntimeInstance>>,
}

impl std::fmt::Debug for RuntimeHandlerManagerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeHandlerManagerInner")
            .field("id", &self.id)
            .field("msg_sender", &self.msg_sender)
            .finish()
    }
}

impl RuntimeHandlerManagerInner {
    fn new(id: &str, msg_sender: Sender<Message>) -> Result<Self> {
        let tracer = KataTracer::new();
        Ok(Self {
            id: id.to_string(),
            msg_sender,
            kata_tracer: Arc::new(Mutex::new(tracer)),
            runtime_instance: None,
        })
    }

    #[instrument]
    async fn init_runtime_handler(
        &mut self,
        sandbox_config: SandboxConfig,
        toml_config: Arc<TomlConfig>,
    ) -> Result<()> {
        info!(sl!(), "new runtime handler {}", &toml_config.runtime.name);
        let runtime_handler = match toml_config.runtime.name.as_str() {
            #[cfg(feature = "linux")]
            name if name == LinuxContainer::name() => LinuxContainer::new_handler(),
            #[cfg(feature = "wasm")]
            name if name == WasmContainer::name() => WasmContainer::new_handler(),
            #[cfg(feature = "virt")]
            name if name == VirtContainer::name() || name.is_empty() => {
                VirtContainer::new_handler()
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported runtime: {}",
                    &toml_config.runtime.name
                ))
            }
        };
        let runtime_instance = runtime_handler
            .new_instance(&self.id, self.msg_sender.clone(), toml_config.clone())
            .await
            .context("new runtime instance")?;

        // initilize the trace subscriber
        if toml_config.runtime.enable_tracing {
            let mut tracer = self.kata_tracer.lock().await;
            if let Err(e) = tracer.trace_setup(
                &self.id,
                &toml_config.runtime.jaeger_endpoint,
                &toml_config.runtime.jaeger_user,
                &toml_config.runtime.jaeger_password,
            ) {
                warn!(sl!(), "failed to setup tracing, {:?}", e);
            }
        }

        // create sandbox
        runtime_instance
            .sandbox
            .clone()
            .create(sandbox_config)
            .await
            .context("create sandbox")?;

        self.runtime_instance = Some(Arc::new(runtime_instance));
        Ok(())
    }

    async fn init_runtime_instance(
        &mut self,
        sandbox_config: SandboxConfig,
        toml_config: TomlConfig,
    ) -> Result<()> {
        self.init_runtime_handler(sandbox_config, Arc::new(toml_config))
            .await
            .context("init runtime handler")?;

        // the sandbox creation can reach here only once and the sandbox is created
        // so we can safely create the shim management socket right now
        // the unwrap here is safe because the runtime handler is correctly created
        let shim_mgmt_svr = MgmtServer::new(
            &self.id,
            self.runtime_instance.as_ref().unwrap().sandbox.clone(),
        )
        .context(ERR_NO_SHIM_SERVER)?;

        tokio::task::spawn(Arc::new(shim_mgmt_svr).run());
        info!(sl!(), "shim management http server starts");

        Ok(())
    }

    fn get_runtime_instance(&self) -> Option<Arc<RuntimeInstance>> {
        self.runtime_instance.clone()
    }

    fn get_kata_tracer(&self) -> Arc<Mutex<KataTracer>> {
        self.kata_tracer.clone()
    }
}

pub struct RuntimeHandlerManager {
    inner: Arc<RwLock<RuntimeHandlerManagerInner>>,
}

// todo: a more detailed impl for fmt::Debug
impl std::fmt::Debug for RuntimeHandlerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeHandlerManager").finish()
    }
}

impl RuntimeHandlerManager {
    pub fn new(id: &str, msg_sender: Sender<Message>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(RuntimeHandlerManagerInner::new(
                id, msg_sender,
            )?)),
        })
    }

    pub async fn cleanup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let sender = inner.msg_sender.clone();
        let sandbox_state = persist::from_disk::<SandboxState>(&inner.id)
            .context("failed to load the sandbox state")?;

        let toml_config = if let Ok(spec) = load_oci_spec() {
            load_toml_config(Some(&spec), None).context("load config")?
        } else {
            TomlConfig::default()
        };

        let sandbox_args = SandboxRestoreArgs {
            sid: inner.id.clone(),
            toml_config,
            sender,
        };
        match sandbox_state.sandbox_type.clone() {
            #[cfg(feature = "linux")]
            name if name == LinuxContainer::name() => {
                // TODO :support linux container (https://github.com/kata-containers/kata-containers/issues/4905)
                return Ok(());
            }
            #[cfg(feature = "wasm")]
            name if name == WasmContainer::name() => {
                // TODO :support wasm container (https://github.com/kata-containers/kata-containers/issues/4906)
                return Ok(());
            }
            #[cfg(feature = "virt")]
            name if name == VirtContainer::name() => {
                if sandbox_args.toml_config.runtime.keep_abnormal {
                    info!(sl!(), "skip cleanup for keep_abnormal");
                    return Ok(());
                }
                let sandbox = VirtSandbox::restore(sandbox_args, sandbox_state)
                    .await
                    .context("failed to restore the sandbox")?;
                sandbox
                    .cleanup()
                    .await
                    .context("failed to cleanup the resource")?;
            }
            _ => {
                return Ok(());
            }
        }

        Ok(())
    }

    async fn get_runtime_instance(&self) -> Result<Arc<RuntimeInstance>> {
        let inner = self.inner.read().await;
        inner
            .get_runtime_instance()
            .ok_or_else(|| anyhow!("runtime not ready"))
    }

    async fn get_kata_tracer(&self) -> Result<Arc<Mutex<KataTracer>>> {
        let inner = self.inner.read().await;
        Ok(inner.get_kata_tracer())
    }

    #[instrument]
    async fn try_init_sandboxed_runtime(&self, sandbox_config: SandboxConfig) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.runtime_instance.is_some() {
            return Ok(());
        }

        init_runtime_config()?;

        let toml_config = load_toml_config(None, None).context("load toml config")?;

        inner
            .init_runtime_instance(sandbox_config, toml_config)
            .await
    }

    async fn try_init_normal_runtime(
        &self,
        spec: oci::Spec,
        state: oci::State,
        options: Option<Vec<u8>>,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.runtime_instance.is_some() {
            return Ok(());
        }

        init_runtime_config()?;

        let toml_config = load_toml_config(Some(&spec), options).context("load toml config")?;

        let sandbox_config = {
            let mut dns: Vec<String> = vec![];
            for m in &spec.mounts {
                if m.destination == DEFAULT_GUEST_DNS_FILE {
                    let contents = fs::read_to_string(&m.source).await?;
                    dns = contents.split('\n').map(|e| e.to_string()).collect();
                }
            }

            let mut network_created = false;
            // set netns to None if we want no network for the VM
            let netns = if toml_config.runtime.disable_new_netns {
                None
            } else {
                let mut netns_path = None;
                if let Some(linux) = &spec.linux {
                    for ns in &linux.namespaces {
                        if ns.r#type.as_str() != oci::NETWORKNAMESPACE {
                            continue;
                        }
                        // get netns path from oci spec
                        if !ns.path.is_empty() {
                            netns_path = Some(ns.path.clone());
                        }
                        // if we get empty netns from oci spec, we need to create netns for the VM
                        else {
                            let ns_name = generate_netns_name();
                            let netns = NetNs::new(ns_name)?;
                            let path = PathBuf::from(netns.path()).to_str().map(|s| s.to_string());
                            info!(sl!(), "the netns path is {:?}", path);
                            netns_path = path;
                            network_created = true;
                        }
                        break;
                    }
                }
                netns_path
            };

            let network_env = SandboxNetworkEnv {
                netns,
                network_created,
            };

            SandboxConfig {
                sandbox_id: String::new(),
                hostname: spec.hostname.clone(),
                dns,
                network_env,
                annotations: spec.annotations.clone(),
                hooks: spec.hooks.clone(),
                state,
            }
        };

        inner
            .init_runtime_instance(sandbox_config, toml_config)
            .await
    }

    #[instrument(parent = &*(ROOTSPAN))]
    pub async fn handler_sandbox_message(&self, req: SandboxRequest) -> Result<SandboxResponse> {
        if let SandboxRequest::CreateSandbox(sandbox_config) = req {
            self.try_init_sandboxed_runtime(*sandbox_config)
                .await
                .context("try init sandboxed runtime")?;

            Ok(SandboxResponse::CreateSandbox)
        } else {
            self.handler_sandbox_request(req)
                .await
                .context("handler request")
        }
    }

    #[instrument(parent = &*(ROOTSPAN))]
    pub async fn handler_task_message(&self, req: TaskRequest) -> Result<TaskResponse> {
        if let TaskRequest::CreateContainer(container_config) = req {
            // get oci spec
            let bundler_path = format!(
                "{}/{}",
                container_config.bundle,
                oci::OCI_SPEC_CONFIG_FILE_NAME
            );
            let spec = oci::Spec::load(&bundler_path).context("load spec")?;
            let state = oci::State {
                version: spec.version.clone(),
                id: container_config.container_id.to_string(),
                status: oci::ContainerState::Creating,
                pid: 0,
                bundle: container_config.bundle.clone(),
                annotations: spec.annotations.clone(),
            };

            self.try_init_normal_runtime(spec.clone(), state, container_config.options.clone())
                .await
                .context("try init normal runtime")?;

            let instance = self
                .get_runtime_instance()
                .await
                .context("get runtime instance")?;

            // start sandbox in order to create container later
            instance
                .sandbox
                .clone()
                .start()
                .await
                .context("start sandbox")?;

            let shim_pid = instance
                .container_manager
                .create_container(container_config, spec)
                .await
                .context("create container")?;

            Ok(TaskResponse::CreateContainer(shim_pid))
        } else {
            self.handler_task_request(req)
                .await
                .context("handler request")
        }
    }

    pub async fn handler_sandbox_request(&self, req: SandboxRequest) -> Result<SandboxResponse> {
        let instance = self
            .get_runtime_instance()
            .await
            .context("get runtime instance")?;
        let sandbox = instance.sandbox.clone();

        match req {
            SandboxRequest::CreateSandbox(req) => Err(anyhow!("Unreachable request {:?}", req)),
            SandboxRequest::StartSandbox(_) => {
                sandbox.start().await.context("start sandbox")?;

                Ok(SandboxResponse::StartSandbox(StartSandboxInfo {
                    pid: std::process::id(),
                    create_time: Some(SystemTime::now()),
                }))
            }
            SandboxRequest::Platform(_) => Ok(SandboxResponse::Platform(PlatformInfo {
                os: std::env::consts::OS.to_string(),
                architecture: std::env::consts::ARCH.to_string(),
            })),
            SandboxRequest::StopSandbox(_) => {
                sandbox.stop().await.context("stop sandbox")?;

                Ok(SandboxResponse::StopSandbox)
            }
            SandboxRequest::WaitSandbox(_) => {
                let exit_info = sandbox.wait().await.context("wait sandbox")?;

                Ok(SandboxResponse::WaitSandbox(exit_info))
            }
            SandboxRequest::SandboxStatus(_) => {
                let status = sandbox.status().await?;

                Ok(SandboxResponse::SandboxStatus(SandboxStatusInfo {
                    sandbox_id: status.sandbox_id,
                    pid: status.pid,
                    state: status.state,
                    created_at: SystemTime::UNIX_EPOCH.checked_add(status.create_at),
                    exited_at: SystemTime::UNIX_EPOCH.checked_add(status.exited_at),
                }))
            }
            SandboxRequest::Ping(_) => Ok(SandboxResponse::Ping),
            SandboxRequest::ShutdownSandbox(_) => {
                sandbox.shutdown().await.context("shutdown sandbox")?;

                Ok(SandboxResponse::ShutdownSandbox)
            }
        }
    }

    #[instrument(parent = &(*ROOTSPAN))]
    pub async fn handler_task_request(&self, req: TaskRequest) -> Result<TaskResponse> {
        let instance = self
            .get_runtime_instance()
            .await
            .context("get runtime instance")?;
        let sandbox = instance.sandbox.clone();
        let cm = instance.container_manager.clone();

        match req {
            TaskRequest::CreateContainer(req) => Err(anyhow!("Unreachable request {:?}", req)),
            TaskRequest::CloseProcessIO(process_id) => {
                cm.close_process_io(&process_id).await.context("close io")?;
                Ok(TaskResponse::CloseProcessIO)
            }
            TaskRequest::DeleteProcess(process_id) => {
                let resp = cm.delete_process(&process_id).await.context("do delete")?;
                Ok(TaskResponse::DeleteProcess(resp))
            }
            TaskRequest::ExecProcess(req) => {
                cm.exec_process(req).await.context("exec")?;
                Ok(TaskResponse::ExecProcess)
            }
            TaskRequest::KillProcess(req) => {
                cm.kill_process(&req).await.context("kill process")?;
                Ok(TaskResponse::KillProcess)
            }
            TaskRequest::ShutdownContainer(req) => {
                if cm.need_shutdown_sandbox(&req).await {
                    sandbox.shutdown().await.context("do shutdown")?;

                    // stop the tracer collector
                    let kata_tracer = self.get_kata_tracer().await.context("get kata tracer")?;
                    let tracer = kata_tracer.lock().await;
                    tracer.trace_end();
                }
                Ok(TaskResponse::ShutdownContainer)
            }
            TaskRequest::WaitProcess(process_id) => {
                let exit_status = cm.wait_process(&process_id).await.context("wait process")?;
                if cm.is_sandbox_container(&process_id).await {
                    sandbox.stop().await.context("stop sandbox")?;
                }
                Ok(TaskResponse::WaitProcess(exit_status))
            }
            TaskRequest::StartProcess(process_id) => {
                let shim_pid = cm
                    .start_process(&process_id)
                    .await
                    .context("start process")?;
                Ok(TaskResponse::StartProcess(shim_pid))
            }

            TaskRequest::StateProcess(process_id) => {
                let state = cm
                    .state_process(&process_id)
                    .await
                    .context("state process")?;
                Ok(TaskResponse::StateProcess(state))
            }
            TaskRequest::PauseContainer(container_id) => {
                cm.pause_container(&container_id)
                    .await
                    .context("pause container")?;
                Ok(TaskResponse::PauseContainer)
            }
            TaskRequest::ResumeContainer(container_id) => {
                cm.resume_container(&container_id)
                    .await
                    .context("resume container")?;
                Ok(TaskResponse::ResumeContainer)
            }
            TaskRequest::ResizeProcessPTY(req) => {
                cm.resize_process_pty(&req).await.context("resize pty")?;
                Ok(TaskResponse::ResizeProcessPTY)
            }
            TaskRequest::StatsContainer(container_id) => {
                let stats = cm
                    .stats_container(&container_id)
                    .await
                    .context("stats container")?;
                Ok(TaskResponse::StatsContainer(stats))
            }
            TaskRequest::UpdateContainer(req) => {
                cm.update_container(req).await.context("update container")?;
                Ok(TaskResponse::UpdateContainer)
            }
            TaskRequest::Pid => Ok(TaskResponse::Pid(cm.pid().await.context("pid")?)),
            TaskRequest::ConnectContainer(container_id) => Ok(TaskResponse::ConnectContainer(
                cm.connect_container(&container_id)
                    .await
                    .context("connect")?,
            )),
        }
    }
}

fn init_runtime_config() -> Result<()> {
    info!(sl!(), "init runtime config");

    #[cfg(feature = "linux")]
    LinuxContainer::init().context("init linux container")?;
    #[cfg(feature = "wasm")]
    WasmContainer::init().context("init wasm container")?;
    #[cfg(feature = "virt")]
    VirtContainer::init().context("init virt container")?;

    Ok(())
}

/// Config override ordering(high to low):
/// 1. podsandbox annotation
/// 2. environment variable
/// 3. shimv2 create task option
/// 4. If above three are not set, then get default path from DEFAULT_RUNTIME_CONFIGURATIONS
/// in kata-containers/src/libs/kata-types/src/config/default.rs, in array order.
fn load_toml_config(spec: Option<&oci::Spec>, option: Option<Vec<u8>>) -> Result<TomlConfig> {
    const KATA_CONF_FILE: &str = "KATA_CONF_FILE";
    let annotation =
        Annotation::new(spec.map_or_else(HashMap::new, |spec| spec.annotations.clone()));
    let config_path = if let Some(path) = annotation.get_sandbox_config_path() {
        path
    } else if let Ok(path) = std::env::var(KATA_CONF_FILE) {
        path
    } else if let Some(option) = option {
        // get rid of the special characters in options to get the config path
        if option.len() > 2 {
            from_utf8(&option[2..])?.to_string()
        } else {
            String::from("")
        }
    } else {
        String::from("")
    };
    info!(sl!(), "get config path {:?}", &config_path);
    let (mut toml_config, _) =
        TomlConfig::load_from_file(&config_path).context("load toml config from file")?;
    annotation.update_config_by_annotation(&mut toml_config)?;
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    // Sandbox sizing information *may* be provided in two scenarios:
    //   1. The upper layer runtime (ie, containerd or crio) provide sandbox sizing information as an annotation
    //	in the 'sandbox container's' spec. This would typically be a scenario where as part of a create sandbox
    //	request the upper layer runtime receives this information as part of a pod, and makes it available to us
    //	for sizing purposes.
    //   2. If this is not a sandbox infrastructure container, but instead a standalone single container (analogous to "docker run..."),
    //	then the container spec itself will contain appropriate sizing information for the entire sandbox (since it is
    //	a single container.
    if let Some(spec) = spec {
        let initial_size_manager =
            InitialSizeManager::new(spec).context("failed to construct static resource manager")?;
        initial_size_manager
            .setup_config(&mut toml_config)
            .context("failed to setup static resource mgmt config")?;
    };

    info!(sl!(), "get config content {:?}", &toml_config);
    Ok(toml_config)
}

// this update the agent-specfic kernel parameters into hypervisor's bootinfo
// the agent inside the VM will read from file cmdline to get the params and function
fn update_agent_kernel_params(config: &mut TomlConfig) -> Result<()> {
    let mut params = vec![];
    if let Ok(kv) = config.get_agent_kernel_params() {
        for (k, v) in kv.into_iter() {
            if let Ok(s) = Param::new(k.as_str(), v.as_str()).to_string() {
                params.push(s);
            }
        }
        if let Some(h) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
            h.boot_info.add_kernel_params(params);
        }
    }
    Ok(())
}
