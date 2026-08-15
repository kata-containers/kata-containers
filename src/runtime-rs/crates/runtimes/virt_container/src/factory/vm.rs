// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc};

use agent::{kata::KataAgent, Agent, AGENT_KATA};
use anyhow::{anyhow, Context, Result};
use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
use kata_types::config::{Agent as AgentConfig, Hypervisor as HypervisorConfig, TomlConfig};
use resource::{cpu_mem::initial_size::InitialSizeManager, ResourceManager};
use runtime_spec;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::channel;
use uuid::Uuid;

use crate::sandbox::VirtSandbox;

const MESSAGE_BUFFER_SIZE: usize = 8;

/// VM is an abstraction of a virtual machine.
#[derive(Clone)]
pub struct TemplateVm {
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
    /// Creates a new TemplateVm instance with the provided components and resources.
    pub fn new(
        id: String,
        hypervisor: Arc<dyn Hypervisor>,
        agent: Arc<dyn Agent>,
        cpu: f32,
        memory: u32,
    ) -> Self {
        Self {
            id,
            hypervisor,
            agent,
            cpu,
            memory,
            cpu_delta: 0,
        }
    }

    /// Initializes the configured hypervisor for Kata.
    async fn new_hypervisor(config: &VmConfig) -> Result<Arc<dyn Hypervisor>> {
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
            _ => Err(anyhow!("Unsupported agent {}", &agent_name)),
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
        }
    }

    /// Creates a new VM based on the provided configuration.
    pub async fn new_vm(config: VmConfig, toml_config: TomlConfig) -> Result<Self> {
        let sid = Uuid::new_v4().to_string();

        let (sender, _receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);

        let hypervisor = Self::new_hypervisor(&config)
            .await
            .context("new hypervisor")?;

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

        let resource_manager = Arc::new(
            ResourceManager::new(
                &sid,
                agent.clone(),
                hypervisor.clone(),
                toml_config_arc,
                initial_size_manager,
            )
            .await
            .context("build resource manager")?,
        );

        let sandbox = VirtSandbox::new(
            &sid,
            sender.clone(),
            agent.clone(),
            hypervisor.clone(),
            resource_manager.clone(),
            sandbox_config,
            factory,
        )
        .await
        .context("build sandbox")?;

        sandbox.start_template().await.context("start template")?;
        info!(sl!(), "VM has been started from template");

        let hypervisor_config = sandbox.get_hypervisor().hypervisor_config().await;
        let vm = TemplateVm::new(
            sandbox.get_sid(),
            sandbox.get_hypervisor(),
            sandbox.get_agent(),
            hypervisor_config.cpu_info.default_vcpus,
            hypervisor_config.memory_info.default_memory,
        );
        Ok(vm)
    }

    /// Stop a VM
    pub async fn stop(&self) -> Result<()> {
        self.hypervisor
            .stop_vm()
            .await
            .map_err(|e| anyhow::anyhow!("failed to stop vm: {}", e))
    }

    /// Remove runtime resources after the VM has stopped.
    pub async fn cleanup(&self) -> Result<()> {
        self.hypervisor.cleanup().await.context("cleanup vm")
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
