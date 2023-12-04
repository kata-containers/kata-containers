// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, path::PathBuf, str::from_utf8, sync::Arc};

use anyhow::{anyhow, Context, Result};
use common::{
    message::Message,
    types::{Request, Response},
    RuntimeHandler, RuntimeInstance, Sandbox, SandboxNetworkEnv,
};
use hypervisor::Param;
use kata_sys_util::spec::load_oci_spec;
use kata_types::{
    annotations::Annotation, config::default::DEFAULT_GUEST_DNS_FILE, config::TomlConfig,
};
#[cfg(feature = "linux")]
use linux_container::LinuxContainer;
use logging::FILTER_RULE;
use netns_rs::NetNs;
use persist::sandbox_persist::Persist;
use resource::{
    cpu_mem::initial_size::InitialSizeManager,
    network::{dan_config_path, generate_netns_name},
};
use shim_interface::shim_mgmt::ERR_NO_SHIM_SERVER;
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

fn convert_string_to_slog_level(string_level: &str) -> slog::Level {
    match string_level {
        "trace" => slog::Level::Trace,
        "debug" => slog::Level::Debug,
        "info" => slog::Level::Info,
        "warn" => slog::Level::Warning,
        "error" => slog::Level::Error,
        "critical" => slog::Level::Critical,
        _ => slog::Level::Info,
    }
}

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
        spec: &oci::Spec,
        state: &oci::State,
        network_env: SandboxNetworkEnv,
        dns: Vec<String>,
        config: Arc<TomlConfig>,
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
            .new_instance(&self.id, self.msg_sender.clone(), config.clone())
            .await
            .context("new runtime instance")?;

        // initilize the trace subscriber
        if config.runtime.enable_tracing {
            let mut tracer = self.kata_tracer.lock().await;
            if let Err(e) = tracer.trace_setup(
                &self.id,
                &config.runtime.jaeger_endpoint,
                &config.runtime.jaeger_user,
                &config.runtime.jaeger_password,
            ) {
                warn!(sl!(), "failed to setup tracing, {:?}", e);
            }
        }

        // start sandbox
        runtime_instance
            .sandbox
            .start(dns, spec, state, network_env)
            .await
            .context("start sandbox")?;
        self.runtime_instance = Some(Arc::new(runtime_instance));
        Ok(())
    }

    #[instrument]
    async fn try_init(
        &mut self,
        spec: &oci::Spec,
        state: &oci::State,
        options: &Option<Vec<u8>>,
    ) -> Result<()> {
        // return if runtime instance has init
        if self.runtime_instance.is_some() {
            return Ok(());
        }

        let mut dns: Vec<String> = vec![];

        #[cfg(feature = "linux")]
        LinuxContainer::init().context("init linux container")?;
        #[cfg(feature = "wasm")]
        WasmContainer::init().context("init wasm container")?;
        #[cfg(feature = "virt")]
        VirtContainer::init().context("init virt container")?;

        for m in &spec.mounts {
            if m.destination == DEFAULT_GUEST_DNS_FILE {
                let contents = fs::read_to_string(&m.source).await?;
                dns = contents.split('\n').map(|e| e.to_string()).collect();
            }
        }

        let config = load_config(spec, options).context("load config")?;

        update_component_log_level(&config);

        let dan_path = dan_config_path(&config, &self.id);
        let mut network_created = false;
        // set netns to None if we want no network for the VM
        let netns = if config.runtime.disable_new_netns {
            None
        } else if dan_path.exists() {
            info!(sl!(), "Do not create a netns due to DAN");
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

        self.init_runtime_handler(spec, state, network_env, dns, Arc::new(config))
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

        let config = if let Ok(spec) = load_oci_spec() {
            load_config(&spec, &None).context("load config")?
        } else {
            TomlConfig::default()
        };

        let sandbox_args = SandboxRestoreArgs {
            sid: inner.id.clone(),
            toml_config: config,
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
    async fn try_init_runtime_instance(
        &self,
        spec: &oci::Spec,
        state: &oci::State,
        options: &Option<Vec<u8>>,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.try_init(spec, state, options).await
    }

    #[instrument(parent = &*(ROOTSPAN))]
    pub async fn handler_message(&self, req: Request) -> Result<Response> {
        if let Request::CreateContainer(container_config) = req {
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

            self.try_init_runtime_instance(&spec, &state, &container_config.options)
                .await
                .context("try init runtime instance")?;
            let instance = self
                .get_runtime_instance()
                .await
                .context("get runtime instance")?;

            let shim_pid = instance
                .container_manager
                .create_container(container_config, spec)
                .await
                .context("create container")?;

            Ok(Response::CreateContainer(shim_pid))
        } else {
            self.handler_request(req).await.context("handler request")
        }
    }

    #[instrument(parent = &(*ROOTSPAN))]
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

                    // stop the tracer collector
                    let kata_tracer = self.get_kata_tracer().await.context("get kata tracer")?;
                    let tracer = kata_tracer.lock().await;
                    tracer.trace_end();
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
/// 2. environment variable
/// 3. shimv2 create task option
/// 4. If above three are not set, then get default path from DEFAULT_RUNTIME_CONFIGURATIONS
/// in kata-containers/src/libs/kata-types/src/config/default.rs, in array order.
#[instrument]
fn load_config(spec: &oci::Spec, option: &Option<Vec<u8>>) -> Result<TomlConfig> {
    const KATA_CONF_FILE: &str = "KATA_CONF_FILE";
    let annotation = Annotation::new(spec.annotations.clone());
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

    // Clone a logger from global logger to ensure the logs in this function get flushed when drop
    let logger = slog::Logger::clone(&slog_scope::logger());

    info!(logger, "get config path {:?}", &config_path);
    let (mut toml_config, _) = TomlConfig::load_from_file(&config_path).context(format!(
        "load TOML config failed (tried {:?})",
        TomlConfig::get_default_config_file_list()
    ))?;
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
    let initial_size_manager =
        InitialSizeManager::new(spec).context("failed to construct static resource manager")?;
    initial_size_manager
        .setup_config(&mut toml_config)
        .context("failed to setup static resource mgmt config")?;

    info!(logger, "get config content {:?}", &toml_config);
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

// this update the log_level of three component: agent, hypervisor, runtime
// according to the settings read from configuration file
fn update_component_log_level(config: &TomlConfig) {
    // Retrieve the log-levels set in configuration file, modify the FILTER_RULE accordingly
    let default_level = String::from("info");
    let agent_level = if let Some(agent_config) = config.agent.get(&config.runtime.agent_name) {
        agent_config.log_level.clone()
    } else {
        default_level.clone()
    };
    let hypervisor_level =
        if let Some(hypervisor_config) = config.hypervisor.get(&config.runtime.hypervisor_name) {
            hypervisor_config.debug_info.log_level.clone()
        } else {
            default_level.clone()
        };
    let runtime_level = config.runtime.log_level.clone();

    // Update FILTER_RULE to apply changes
    FILTER_RULE.rcu(|inner| {
        let mut updated_inner = HashMap::new();
        updated_inner.clone_from(inner);
        updated_inner.insert(
            "runtimes".to_string(),
            convert_string_to_slog_level(&runtime_level),
        );
        updated_inner.insert(
            "agent".to_string(),
            convert_string_to_slog_level(&agent_level),
        );
        updated_inner.insert(
            "hypervisor".to_string(),
            convert_string_to_slog_level(&hypervisor_level),
        );
        updated_inner
    });
}
