#![allow(unused_mut)]
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;

use std::collections::HashMap;
use std::sync::Arc;

use kata_types::config::default;
use kata_types::config::Agent as AgentConfig;
use kata_types::config::Hypervisor as HypervisorConfig;
use kata_types::config::TomlConfig;
use serde::{Deserialize, Serialize};

use agent::{kata::KataAgent, Agent, AGENT_KATA};
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
use resource::{cpu_mem::initial_size::InitialSizeManager, ResourceManager};

use crate::factory;
use crate::sandbox::VirtSandbox;
use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};
use runtime_spec;
use slog::error;
use tokio::sync::mpsc::channel;
use uuid::Uuid;
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "vm"))
    };
}

/// VM is an abstraction of a virtual machine.
#[allow(dead_code)]
#[derive(Clone)]
pub struct VM {
    /// The hypervisor responsible for managing the virtual machine lifecycle.
    pub hypervisor: Arc<dyn Hypervisor>,

    /// The guest agent that communicates with the virtual machine.
    pub agent: Arc<dyn Agent>,

    // // / Persistent storage driver to save and restore VM state.
    // pub store: Box<dyn PersistDriver>,
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
        // remote_hypervisor_socket
        if !conf.remote_info.hypervisor_socket.is_empty() {
            return Ok(());
        }

        // kernel_path
        if conf.boot_info.kernel.is_empty() {
            let e = anyhow!("Missing kernel path");
            error!(sl!(), "{:#?}", e);
            return Err(e);
        }

        // In Secure Execution mode, the `image` and `initrd` settings are prohibited from being configured
        if conf.security_info.confidential_guest
            && conf.machine_info.machine_type == "s390-ccw-virtio"
        // QemuCCWVirtio
        {
            if !conf.boot_info.image.is_empty() || !conf.boot_info.initrd.is_empty() {
                let e = anyhow!("Neither the image or initrd path may be set for Secure Execution");
                error!(sl!(), "{:#?}", e);
                return Err(e);
            }
        } else if conf.boot_info.image.is_empty() && conf.boot_info.initrd.is_empty() {
            let e = anyhow!("Missing image and initrd path");
            error!(sl!(), "{:#?}", e);
            return Err(e);
        } else if !conf.boot_info.image.is_empty() && !conf.boot_info.initrd.is_empty() {
            let e = anyhow!("Image and initrd path cannot be both set");
            error!(sl!(), "{:#?}", e);
            return Err(e);
        }

        // template valid todo 

        // num_vcpus_f
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
        // let cpus = num_cpus::get() as u32;
        if conf.cpu_info.default_maxvcpus == 0
            || conf.cpu_info.default_maxvcpus > default::MAX_QEMU_VCPUS
        {
            conf.cpu_info.default_maxvcpus = default::MAX_QEMU_VCPUS;
        }

        // msize9p
        if conf.shared_fs.shared_fs.as_deref() != Some("virtio-fs") && conf.shared_fs.msize_9p == 0
        {
            conf.shared_fs.msize_9p = default::MAX_SHARED_9PFS_SIZE_MB;
        }

        Ok(())
    }
}

#[allow(dead_code)]
impl VM {
    // Initializes the QEMU hypervisor for Kata
    // Refactored the function to support only QEMU, as access to the private function in `virt_container::lib::new_hypervisor()` is unavailable.
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
    // analogous to `virt_container::lib::new_agent()`.
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
        }
    }

    // Creates a new VM based on the provided configuration.
    #[allow(unused_variables)]
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
        // get uds from hypervisor and get config from toml_config
        let agent = Self::new_agent(&config).context("new agent")?;

        // sandbox_config
        let sandbox_config = Self::new_empty_sandbox_config();

        let mut initial_size_manager = InitialSizeManager::new_from(&sandbox_config.annotations)
            .context("failed to construct static resource manager")?;

        // We need to update the `toml_config` with runtime information, 
        // but due to ownership issues with the variables, we cannot pass them as parameters.
        // Therefore, for now, we directly set the `slot` and `maxmemory` values in the configuration file to non-zero.

        // initial_size_manager
        //     .setup_config(&mut toml_config)
        //     .context("failed to setup static resource mgmt config")?;

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

        // info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sandbox));

        let sb = sandbox.unwrap();

        match sb.start_template().await {
            Ok(_) => {
                info!(sl!(), "vm::new_vm():"; "sb" => format!("{:?}", sb));
            }
            Err(e) => {
                error!(sl!(), "vm::new_vm(): sandbox start failed: {}", e);
            }
        }
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

    pub async fn stop(&self) -> Result<()> {

        // Stop qemu process
        self.hypervisor
            .stop_vm()
            .await
            .map_err(|e| anyhow::anyhow!("failed to stop vm: {}", e))?;

        // To be implemented: Remove the control resources from the cgroups, 
        // similar to how it's done in `store.Destroy()` in Go.

        Ok(())
    }

    // Disconnect the gRPC connection between the Kata agent and the VM
    pub async fn disconnect(&self) -> Result<()> {
        info!(sl!(), "vm::disconnect(): begin");
        // To be implemented
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
