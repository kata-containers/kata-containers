// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use slog::error;
use tokio::sync::mpsc::channel;
use uuid::Uuid;

use kata_types::config::{
    default, Agent as AgentConfig, Hypervisor as HypervisorConfig, TomlConfig,
};

use agent::{kata::KataAgent, Agent, AGENT_KATA};
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
use resource::{cpu_mem::initial_size::InitialSizeManager, ResourceManager};

use crate::sandbox::VirtSandbox;
use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};

use runtime_spec;

/// VM is an abstraction of a virtual machine.
#[derive(Clone)]
pub struct VM {
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

/// VMConfig holds all configuration information required to start a new VM instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VMConfig {
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

impl VMConfig {
    pub fn valid(&mut self) -> Result<()> {
        VMConfig::validate_hypervisor_config(&mut self.hypervisor_config)
    }

    pub fn validate_hypervisor_config(conf: &mut HypervisorConfig) -> Result<()> {
        // remote hypervisor_socket
        if !conf.remote_info.hypervisor_socket.is_empty() {
            return Ok(());
        }

        // kernel_path
        if conf.boot_info.kernel.is_empty() {
            let e = anyhow!("Missing kernel path");
            error!(sl!(), "{:#?}", e);
            return Err(e);
        }

        let secure = conf.security_info.confidential_guest
            && conf.machine_info.machine_type == "s390-ccw-virtio";

        let has_image = !conf.boot_info.image.is_empty();
        let has_initrd = !conf.boot_info.initrd.is_empty();

        if secure {
            if has_image || has_initrd {
                return Err(anyhow!(
                    "Secure Execution mode does not allow image or initrd"
                ));
            }
            return Ok(());
        }

        match (has_image, has_initrd) {
            (false, false) => Err(anyhow!("Missing image and initrd path")),
            (true, true) => Err(anyhow!("Image and initrd path cannot both be set")),
            _ => Ok(()),
        }?;

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
        } else if conf.blockdev_info.block_device_driver == "virtio-blk"
            && conf.machine_info.machine_type == "s390-ccw-virtio"
        {
            conf.blockdev_info.block_device_driver = "virtio-blk-ccw".to_string();
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

impl VM {
    // Initializes the QEMU hypervisor for Kata
    async fn new_hypervisor(config: &VMConfig) -> Result<Arc<dyn Hypervisor>> {
        let hypervisor: Arc<dyn Hypervisor> = match config.hypervisor_name.as_str() {
            HYPERVISOR_QEMU => {
                let h = Qemu::new();
                h.set_hypervisor_config(config.hypervisor_config.clone())
                    .await;
                Arc::new(h)
            }
            _ => return Err(anyhow!("Unsupported hypervisor {}", config.hypervisor_name)),
        };
        Ok(hypervisor)
    }

    // Initializes the Kata agent, handling necessary configurations and setup
    fn new_agent(config: &VMConfig) -> Result<Arc<KataAgent>> {
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

    // Create an empty `sandbox_config` structure
    fn new_empty_sandbox_config() -> SandboxConfig {
        SandboxConfig {
            sandbox_id: String::new(),
            hostname: String::new(),
            dns: Vec::new(),
            network_env: SandboxNetworkEnv {
                netns: None,
                network_created: false,
            },
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

    // Creates a new VM based on the provided configuration.
    pub async fn new_vm(config: VMConfig, toml_config: TomlConfig) -> Result<Self> {
        let sid = Uuid::new_v4().to_string();

        // msg_sender
        const MESSAGE_BUFFER_SIZE: usize = 8;
        let (sender, _receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);

        // hypervisor
        let hypervisor = Self::new_hypervisor(&config)
            .await
            .context("new hypervisor")?;

        // agent
        let agent = Self::new_agent(&config).context("new agent")?;

        // sandbox_config
        let sandbox_config = Self::new_empty_sandbox_config();

        let initial_size_manager = InitialSizeManager::new_from(&sandbox_config.annotations)
            .context("failed to construct static resource manager")?;

        // We need to update the `toml_config` with runtime information,
        // but due to ownership issues with the variables, we cannot pass them as parameters.
        // Therefore, for now, we directly set the `slot` and `maxmemory` values in the configuration file to non-zero.

        let factory = toml_config.factory.clone();

        let toml_config_arc = Arc::new(toml_config);

        // resource_manager
        let resource_manager = Arc::new(
            ResourceManager::new(
                &sid,
                agent.clone(),
                hypervisor.clone(),
                toml_config_arc,
                initial_size_manager,
            )
            .await?,
        );

        // sandbox_config
        let sandbox = VirtSandbox::new(
            &sid,
            sender.clone(),
            agent.clone(),
            hypervisor.clone(),
            resource_manager.clone(),
            sandbox_config,
            factory,
        )
        .await;

        let sb = sandbox.unwrap();
        sb.start_template()
            .await
            .context("vm::new_vm(): sandbox start failed")?;
        info!(sl!(), "vm::new_vm(): VM start successfully");

        let hypervisor_config = sb.hypervisor.hypervisor_config().await;

        let vm = VM {
            id: sb.sid,
            hypervisor: sb.hypervisor.clone(),
            agent: sb.agent.clone(),
            cpu: hypervisor_config.cpu_info.default_vcpus,
            memory: hypervisor_config.memory_info.default_memory,
            cpu_delta: 0,
        };
        Ok(vm)
    }

    // Stop a VM
    pub async fn stop(&self) -> Result<()> {
        self.hypervisor
            .stop_vm()
            .await
            .map_err(|e| anyhow::anyhow!("failed to stop vm: {}", e))?;
        Ok(())
    }

    // Disconnect agent
    pub async fn disconnect(&self) -> Result<()> {
        info!(sl!(), "vm::disconnect(): begin");
        self.agent
            .disconnect()
            .await
            .context("vm disconnect failed")?;
        Ok(())
    }

    // Pause a VM.
    pub async fn pause(&self) -> Result<()> {
        info!(sl!(), "vm::pause(): start");
        self.hypervisor.pause_vm().await
    }

    // Save a VM to persistent disk.
    pub async fn save(&self) -> Result<()> {
        info!(sl!(), "vm::save(): start");
        self.hypervisor.save_vm().await
    }

    // Resume resumes a paused VM.
    pub async fn resume(&self) -> Result<()> {
        info!(sl!(), "vm::resume(): start");
        self.hypervisor.resume_vm().await
    }
}
