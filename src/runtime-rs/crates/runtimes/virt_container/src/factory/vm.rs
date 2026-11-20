// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process;
use std::sync::Arc;

use agent::{kata::KataAgent, Agent, AGENT_KATA};
use anyhow::{anyhow, Context, Result};
use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};
#[cfg(all(feature = "dragonball", target_arch = "x86_64"))]
use hypervisor::{dragonball::Dragonball, HYPERVISOR_DRAGONBALL};
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
use kata_types::config::{Agent as AgentConfig, Hypervisor as HypervisorConfig, TomlConfig};
use resource::{cpu_mem::initial_size::InitialSizeManager, ResourceManager};
use runtime_spec;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::sandbox::VirtSandbox;

const CGROUP2_ROOT: &str = "/sys/fs/cgroup";

fn launch_cgroup() -> Result<Option<PathBuf>> {
    let root = Path::new(CGROUP2_ROOT);
    if !root.join("cgroup.controllers").exists() {
        return Ok(None);
    }

    let membership = fs::read_to_string("/proc/self/cgroup").context("read factory cgroup")?;
    let path = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::/"))
        .map(Path::new)
        .context("resolve factory cgroup")?;
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(anyhow!("factory cgroup is outside the mounted hierarchy"));
    }

    let procs = root.join(path).join("cgroup.procs");
    if !contains_factory_process(&procs)? {
        return Err(anyhow!(
            "{} does not contain the factory process; cgroup mount and namespace may not match",
            procs.display()
        ));
    }
    Ok(Some(procs))
}

fn contains_factory_process(path: &Path) -> Result<bool> {
    let members = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let pid = process::id().to_string();
    Ok(members.lines().any(|member| member == pid))
}

fn restore_launch_cgroup(original: Option<&Path>) -> Result<()> {
    let Some(path) = original else {
        return Ok(());
    };
    if !contains_factory_process(path)? {
        OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|mut procs| procs.write_all(b"0\n"))
            .with_context(|| format!("return factory process to {}", path.display()))?;
    }
    Ok(())
}

/// VM is an abstraction of a virtual machine.
#[derive(Clone)]
pub struct TemplateVm {
    /// Sandbox that owns the VM and its host-side resources.
    sandbox: VirtSandbox,
    launch_cgroup: Option<PathBuf>,

    /// The hypervisor responsible for managing the virtual machine lifecycle.
    pub hypervisor: Arc<dyn Hypervisor>,

    /// The guest agent that communicates with the virtual machine.
    pub agent: Arc<dyn Agent>,

    /// Unique identifier of the virtual machine.
    pub id: String,

    /// Number of vCPUs assigned to the VM.
    pub cpu: f32,

    /// Amount of memory (in MB) assigned to the VM.
    pub memory: u32,

    /// Tracks the difference in vCPU count since last update.
    pub cpu_delta: i32,
}

/// VmConfig holds all configuration information required to start a new VM instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmConfig {
    /// Type of hypervisor to be used (e.g., qemu, clh).
    #[serde(default)]
    pub hypervisor_name: String,
    #[serde(default)]
    pub agent_name: String,
    /// Configuration for the guest agent.
    #[serde(default)]
    pub agent_config: AgentConfig,

    /// Configuration for the hypervisor.
    #[serde(default)]
    pub hypervisor_config: HypervisorConfig,
}

impl VmConfig {
    pub fn new(toml_config: &TomlConfig) -> Self {
        let hypervisor_name = toml_config.runtime.hypervisor_name.clone();
        let agent_name = toml_config.runtime.agent_name.clone();

        let hypervisor_config = toml_config
            .hypervisor
            .get(&hypervisor_name)
            .cloned()
            .unwrap_or_default();

        let agent_config = toml_config
            .agent
            .get(&agent_name)
            .cloned()
            .unwrap_or_default();

        VmConfig {
            hypervisor_name,
            agent_name,
            hypervisor_config,
            agent_config,
        }
    }
}

impl TemplateVm {
    /// Creates a new TemplateVm instance from its owning sandbox.
    fn new(sandbox: VirtSandbox, cpu: f32, memory: u32, launch_cgroup: Option<PathBuf>) -> Self {
        let id = sandbox.get_sid();
        let hypervisor = sandbox.get_hypervisor();
        let agent = sandbox.get_agent();
        Self {
            sandbox,
            launch_cgroup,
            id,
            hypervisor,
            agent,
            cpu,
            memory,
            cpu_delta: 0,
        }
    }

    /// Initializes the configured hypervisor for Kata.
    async fn new_hypervisor(
        config: &VmConfig,
        _toml_config: &TomlConfig,
    ) -> Result<Arc<dyn Hypervisor>> {
        let hypervisor: Arc<dyn Hypervisor> = match config.hypervisor_name.as_str() {
            HYPERVISOR_QEMU => {
                let h = Qemu::new();
                h.set_hypervisor_config(config.hypervisor_config.clone())
                    .await;
                Arc::new(h)
            }
            #[cfg(all(
                feature = "cloud-hypervisor",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            hypervisor::HYPERVISOR_NAME_CH => {
                let h = hypervisor::ch::CloudHypervisor::new();
                h.set_hypervisor_config(config.hypervisor_config.clone())
                    .await;
                Arc::new(h)
            }
            #[cfg(all(feature = "dragonball", target_arch = "x86_64"))]
            HYPERVISOR_DRAGONBALL => {
                let h = Dragonball::new();
                h.set_hypervisor_config(config.hypervisor_config.clone())
                    .await;
                if _toml_config.runtime.use_passfd_io {
                    h.set_passfd_listener_port(_toml_config.runtime.passfd_listener_port)
                        .await;
                }
                Arc::new(h)
            }
            _ => return Err(anyhow!("Unsupported hypervisor {}", config.hypervisor_name)),
        };
        Ok(hypervisor)
    }

    /// Initializes the Kata agent, handling necessary configurations and setup
    fn new_agent(config: &VmConfig) -> Result<Arc<KataAgent>> {
        let agent_name = &config.agent_name;
        let agent_config = config.agent_config.clone();

        match agent_name.as_str() {
            AGENT_KATA => {
                let agent = KataAgent::new(agent_config.clone());
                Ok(Arc::new(agent))
            }
            _ => Err(anyhow!("Unsupported agent {}", agent_name)),
        }
    }

    /// Create an empty `sandbox_config` structure
    fn new_empty_sandbox_config() -> SandboxConfig {
        SandboxConfig {
            sandbox_id: String::new(),
            hostname: String::new(),
            dns: Vec::new(),
            network_env: SandboxNetworkEnv::default(),
            annotations: HashMap::default(),
            hooks: None,
            state: runtime_spec::State {
                version: Default::default(),
                id: String::new(),
                status: runtime_spec::ContainerState::Creating,
                pid: 0,
                bundle: String::new(),
                annotations: Default::default(),
            },
            shm_size: 0,
            config_path: None,
        }
    }

    /// Prepares the sandbox before starting the template VM.
    pub(super) async fn prepare(
        config: VmConfig,
        toml_config: TomlConfig,
        sender: Sender<Message>,
    ) -> Result<Self> {
        let sid = Uuid::new_v4().to_string();

        let hypervisor = Self::new_hypervisor(&config, &toml_config)
            .await
            .context("new hypervisor")?;
        let hypervisor_config = hypervisor.hypervisor_config().await;

        let agent = Self::new_agent(&config).context("new agent")?;

        let sandbox_config = Self::new_empty_sandbox_config();

        let initial_size_manager = InitialSizeManager::new_from(&sandbox_config.annotations)
            .context("failed to construct static resource manager")?;

        // We need to update the `toml_config` with runtime information,
        // but due to ownership issues with the variables, we cannot
        // pass them as parameters.
        // Therefore, for now, we directly set the `slot` and
        // `maxmemory` values in the configuration file to non-zero.

        let factory = toml_config.get_factory();

        let toml_config_arc = Arc::new(toml_config);

        // The resource manager moves the caller into the template sandbox's cgroup.
        let launch_cgroup = launch_cgroup()?;
        let resource_manager = ResourceManager::new(
            &sid,
            agent.clone(),
            hypervisor.clone(),
            toml_config_arc,
            initial_size_manager,
        )
        .await;
        let resource_manager = match resource_manager {
            Ok(manager) => Arc::new(manager),
            Err(err) => {
                restore_launch_cgroup(launch_cgroup.as_deref())
                    .with_context(|| format!("build resource manager: {err:#}"))?;
                return Err(err).context("build resource manager");
            }
        };

        let sandbox = VirtSandbox::new(
            &sid,
            sender,
            agent,
            hypervisor,
            resource_manager,
            sandbox_config,
            factory,
        )
        .await
        .context("build sandbox")?;

        Ok(Self::new(
            sandbox,
            hypervisor_config.cpu_info.default_vcpus,
            hypervisor_config.memory_info.default_memory,
            launch_cgroup,
        ))
    }

    pub(super) async fn start(&self) -> Result<()> {
        self.sandbox
            .start_template()
            .await
            .context("start template")?;
        info!(sl!(), "template VM has been started");

        let address = self
            .hypervisor
            .get_agent_socket()
            .await
            .context("get template VM agent socket")?;
        self.agent
            .start(&address)
            .await
            .context("connect to template VM agent")
    }

    /// Stop a VM
    pub async fn stop(&self) -> Result<()> {
        self.sandbox.stop().await.context("stop template sandbox")
    }

    /// Remove sandbox-owned and hypervisor resources after the VM has stopped.
    pub async fn cleanup(&self) -> Result<()> {
        self.sandbox
            .cleanup()
            .await
            .context("cleanup template sandbox")
    }

    /// Stop the template VM and release all resources created for its sandbox.
    pub async fn teardown(&self) -> Result<()> {
        self.agent.stop().await;

        let stop_result = self.sandbox.stop().await;
        if !self.vmm_exit_confirmed() {
            stop_result.context("stop template sandbox during teardown")?;
            return Err(anyhow!("VMM exit unconfirmed; preserving template sandbox"));
        }
        // Sandbox cleanup can kill remaining processes in the sandbox cgroup.
        restore_launch_cgroup(self.launch_cgroup.as_deref()).with_context(
            || match &stop_result {
                Ok(()) => "tear down template sandbox".to_string(),
                Err(err) => format!("stop template sandbox during teardown: {err:#}"),
            },
        )?;
        let cleanup_result = self.sandbox.cleanup().await;

        match (stop_result, cleanup_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(stop_error), Ok(())) => {
                Err(stop_error).context("stop template sandbox during teardown")
            }
            (Ok(()), Err(cleanup_error)) => {
                Err(cleanup_error).context("cleanup template sandbox during teardown")
            }
            (Err(stop_error), Err(cleanup_error)) => Err(anyhow!(
                "stop template sandbox during teardown: {stop_error:#}; \
                 cleanup template sandbox during teardown: {cleanup_error:#}"
            )),
        }
    }

    pub(super) fn vmm_exit_confirmed(&self) -> bool {
        self.sandbox.vmm_exit_confirmed()
    }

    /// Disconnect agent
    pub async fn disconnect(&self) -> Result<()> {
        self.agent.disconnect().await.context("disconnect vm")
    }

    /// Pause a VM.
    pub async fn pause(&self) -> Result<()> {
        self.hypervisor.pause_vm().await.context("pause vm")
    }

    /// Save a VM to persistent disk.
    pub async fn save(&self) -> Result<()> {
        self.hypervisor.save_vm().await.context("save vm")
    }

    /// Resume resumes a paused VM.
    pub async fn resume(&self) -> Result<()> {
        self.hypervisor.resume_vm().await.context("resume vm")
    }
}
