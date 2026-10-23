// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc};

use agent::{kata::KataAgent, Agent, AgentManager, AGENT_KATA};
use anyhow::{anyhow, Context, Result};
use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};
#[cfg(all(feature = "dragonball", target_arch = "x86_64"))]
use hypervisor::{dragonball::Dragonball, HYPERVISOR_DRAGONBALL};
use hypervisor::{qemu::Qemu, Hypervisor, Param, HYPERVISOR_QEMU};
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
    /// Sandbox that owns the VM and its host-side resources.
    sandbox: VirtSandbox,

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
    pub fn new(sandbox: VirtSandbox, cpu: f32, memory: u32) -> Self {
        let id = sandbox.get_sid();
        let hypervisor = sandbox.get_hypervisor();
        let agent = sandbox.get_agent();
        Self {
            sandbox,
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
        toml_config: &TomlConfig,
    ) -> Result<Arc<dyn Hypervisor>> {
        let mut hypervisor_config = config.hypervisor_config.clone();
        let agent_params = toml_config
            .get_agent_kernel_params()?
            .into_iter()
            .map(|(key, value)| Param::new(&key, &value).to_string())
            .collect::<Result<Vec<_>>>()?;
        hypervisor_config.boot_info.add_kernel_params(agent_params);

        let hypervisor: Arc<dyn Hypervisor> = match config.hypervisor_name.as_str() {
            HYPERVISOR_QEMU => {
                let h = Qemu::new();
                h.set_hypervisor_config(hypervisor_config).await;
                Arc::new(h)
            }
            #[cfg(all(
                feature = "cloud-hypervisor",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            hypervisor::HYPERVISOR_NAME_CH => {
                let h = hypervisor::ch::CloudHypervisor::new();
                h.set_hypervisor_config(hypervisor_config).await;
                Arc::new(h)
            }
            #[cfg(all(feature = "dragonball", target_arch = "x86_64"))]
            HYPERVISOR_DRAGONBALL => {
                let h = Dragonball::new();
                h.set_hypervisor_config(hypervisor_config).await;
                if toml_config.runtime.use_passfd_io {
                    h.set_passfd_listener_port(toml_config.runtime.passfd_listener_port)
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
        }
    }

    /// Creates a new VM based on the provided configuration.
    pub async fn new_vm(config: VmConfig, toml_config: TomlConfig) -> Result<Self> {
        let sid = Uuid::new_v4().to_string();

        let (sender, _receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);

        let hypervisor = Self::new_hypervisor(&config, &toml_config)
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

        if let Err(start_error) = sandbox.start_template().await {
            if let Err(teardown_error) = Self::teardown_sandbox(&sandbox).await {
                warn!(
                    sl!(),
                    "failed to tear down template VM after startup failed: {}", teardown_error
                );
            }
            return Err(start_error).context("start template");
        }
        info!(sl!(), "template VM has been started");

        let hypervisor_config = sandbox.get_hypervisor().hypervisor_config().await;
        let vm = TemplateVm::new(
            sandbox,
            hypervisor_config.cpu_info.default_vcpus,
            hypervisor_config.memory_info.default_memory,
        );

        let readiness_result: Result<()> = async {
            let address = hypervisor
                .get_agent_socket()
                .await
                .context("get template VM agent socket")?;
            agent
                .start(&address)
                .await
                .context("connect to template VM agent")
        }
        .await;

        if let Err(readiness_error) = readiness_result {
            if let Err(teardown_error) = vm.teardown().await {
                warn!(
                    sl!(),
                    "failed to tear down template VM after agent readiness failed: {}",
                    teardown_error
                );
            }
            return Err(readiness_error);
        }

        Ok(vm)
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
        Self::teardown_sandbox(&self.sandbox).await
    }

    async fn teardown_sandbox(sandbox: &VirtSandbox) -> Result<()> {
        sandbox.get_agent().stop().await;

        let stop_result = sandbox.stop().await;
        let cleanup_result = sandbox.cleanup().await;

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

#[cfg(test)]
mod tests {
    use super::*;

    async fn assert_factory_agent_debug_kernel_params(hypervisor_name: &str) {
        for debug in [false, true] {
            for debug_console_enabled in [false, true] {
                let mut toml_config = TomlConfig::default();
                toml_config.runtime.hypervisor_name = hypervisor_name.to_string();
                toml_config.runtime.agent_name = AGENT_KATA.to_string();
                toml_config.agent.insert(
                    AGENT_KATA.to_string(),
                    AgentConfig {
                        debug,
                        debug_console_enabled,
                        ..Default::default()
                    },
                );
                let mut hypervisor_config = HypervisorConfig::default();
                hypervisor_config.boot_info.kernel_params = "existing=value".to_string();
                toml_config
                    .hypervisor
                    .insert(hypervisor_name.to_string(), hypervisor_config);

                let config = VmConfig::new(&toml_config);
                let hypervisor = TemplateVm::new_hypervisor(&config, &toml_config)
                    .await
                    .unwrap();
                let boot_config = hypervisor.hypervisor_config().await;
                let params: Vec<_> = boot_config
                    .boot_info
                    .kernel_params
                    .split_whitespace()
                    .collect();

                assert!(params.contains(&"existing=value"));
                assert_eq!(params.contains(&"agent.log=debug"), debug);
                assert_eq!(
                    params.contains(&"agent.debug_console"),
                    debug_console_enabled
                );
                assert_eq!(
                    params.contains(&"agent.debug_console_vport=1026"),
                    debug_console_enabled
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_agent_debug_kernel_params() {
        assert_factory_agent_debug_kernel_params(HYPERVISOR_QEMU).await;
    }

    #[cfg(all(
        feature = "cloud-hypervisor",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[tokio::test]
    async fn test_factory_clh_agent_debug_kernel_params() {
        assert_factory_agent_debug_kernel_params(hypervisor::HYPERVISOR_NAME_CH).await;
    }
}
