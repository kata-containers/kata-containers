// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc};

use agent::{kata::KataAgent, Agent, AGENT_KATA};
use anyhow::{anyhow, Context, Result};
use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};
use hypervisor::device::driver::{VIRTIO_BLOCK_CCW, VIRTIO_BLOCK_PCI};
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
use kata_types::config::{
    default, Agent as AgentConfig, Hypervisor as HypervisorConfig, TomlConfig,
};
use kata_types::machine_type::MACHINE_TYPE_S390X_TYPE;
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
    /// Type of hypervisor to be used (e.g., qemu, cloud-hypervisor).
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

    /// Validates boot configuration based on security mode
    fn validate_boot_configuration(conf: &HypervisorConfig) -> Result<()> {
        let is_secure_execution = conf.security_info.confidential_guest
            && conf.machine_info.machine_type == MACHINE_TYPE_S390X_TYPE;

        let has_image = !conf.boot_info.image.is_empty();
        let has_initrd = !conf.boot_info.initrd.is_empty();

        // Secure execution mode does not allow image or initrd
        if is_secure_execution {
            if has_image || has_initrd {
                return Err(anyhow!(
                    "secure execution mode does not allow image or initrd"
                ));
            }
            return Ok(());
        }

        // Standard mode: must have exactly one of image or initrd
        if !has_image && !has_initrd {
            return Err(anyhow!("missing image and initrd path"));
        }

        if has_image && has_initrd {
            return Err(anyhow!("image and initrd path cannot both be set"));
        }

        Ok(())
    }

    pub fn validate_hypervisor_config(conf: &mut HypervisorConfig) -> Result<()> {
        // remote hypervisor_socket
        if !conf.remote_info.hypervisor_socket.is_empty() {
            return Ok(());
        }

        // kernel_path
        if conf.boot_info.kernel.is_empty() {
            return Err(anyhow!("missing kernel path"));
        }

        // Validate boot configuration based on security mode
        Self::validate_boot_configuration(conf)?;

        // vcpus
        if conf.cpu_info.default_vcpus == 0.0 {
            conf.cpu_info.default_vcpus = default::DEFAULT_GUEST_VCPUS as f32;
        }

        // memory_size
        if conf.memory_info.default_memory == 0 {
            conf.memory_info.default_memory = default::DEFAULT_QEMU_MEMORY_SIZE_MB;
        }

        // default_bridges
        if conf.device_info.default_bridges == 0 {
            conf.device_info.default_bridges = default::DEFAULT_QEMU_PCI_BRIDGES;
        }

        // block_device_driver
        if conf.blockdev_info.block_device_driver.is_empty() {
            conf.blockdev_info.block_device_driver = default::DEFAULT_BLOCK_DEVICE_TYPE.to_string();
        } else if conf.blockdev_info.block_device_driver == VIRTIO_BLOCK_PCI
            && conf.machine_info.machine_type == MACHINE_TYPE_S390X_TYPE
        {
            conf.blockdev_info.block_device_driver = VIRTIO_BLOCK_CCW.to_string();
        }

        // default_maxvcpus
        if conf.cpu_info.default_maxvcpus == 0
            || conf.cpu_info.default_maxvcpus > default::MAX_QEMU_VCPUS
        {
            conf.cpu_info.default_maxvcpus = default::MAX_QEMU_VCPUS;
        }

        Ok(())
    }
}

impl TemplateVm {
    /// Creates a new TemplateVm instance with the provided components and resources.
    /// Currently, only QEMU is supported; other hypervisors are not yet implemented.
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

    /// Initializes the QEMU hypervisor for Kata
    async fn new_hypervisor(config: &VmConfig) -> Result<Arc<dyn Hypervisor>> {
        let hypervisor: Arc<dyn Hypervisor> = match config.hypervisor_name.as_str() {
            HYPERVISOR_QEMU => {
                let h = Qemu::new();
                h.set_hypervisor_config(config.hypervisor_config.clone())
                    .await;
                Arc::new(h)
            }
            // TODO: Add support for additional hypervisors or proper error handling here.
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
