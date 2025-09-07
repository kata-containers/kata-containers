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

use common::{message::Message, types::SandboxConfig, Sandbox, SandboxNetworkEnv};
use runtime_spec;
use slog::error;
use tokio::sync::mpsc::channel;
use crate::sandbox::VirtSandbox;
use crate::factory;
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
        // 1. remote_hypervisor_socket 对应 remote_info.hypervisor_socket
        if !conf.remote_info.hypervisor_socket.is_empty() {
            return Ok(());
        }

        // 2. kernel_path 校验
        if conf.boot_info.kernel.is_empty() {
            let e = anyhow!("Missing kernel path");
            error!(sl!(), "{:#?}", e);
            return Err(e);
        }

        // 3. Secure Execution 模式下 image/initrd 禁止设置
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

        // 4. template 配置校验
        // if let Err(e) = conf.boot_info.validate_template_config() {
        //     error!(sl!(), "{:#?}", e);
        //     return Err(e);
        // }

        // 5. num_vcpus_f 填默认值
        if conf.cpu_info.default_vcpus == 0.0 {
            conf.cpu_info.default_vcpus = default::DEFAULT_GUEST_VCPUS as f32;
        }

        // 6. memory_size 填默认值
        if conf.memory_info.default_memory == 0 {
            conf.memory_info.default_memory = default::DEFAULT_QEMU_MEMORY_SIZE_MB;
        }

        // 7. default_bridges 填默认值
        if conf.device_info.default_bridges == 0 {
            conf.device_info.default_bridges = default::DEFAULT_QEMU_PCI_BRIDGES;
        }

        // 8. block_device_driver 逻辑修正
        if conf.blockdev_info.block_device_driver.is_empty() {
            conf.blockdev_info.block_device_driver = default::DEFAULT_BLOCK_DEVICE_TYPE.to_string();
        } else if conf.blockdev_info.block_device_driver == "virtio-blk"
            && conf.machine_info.machine_type == "s390-ccw-virtio"
        {
            conf.blockdev_info.block_device_driver = "virtio-blk-ccw".to_string();
        }

        // 9. default_maxvcpus 限定上限
        // let cpus = num_cpus::get() as u32;
        if conf.cpu_info.default_maxvcpus == 0
            || conf.cpu_info.default_maxvcpus > default::MAX_QEMU_VCPUS
        {
            conf.cpu_info.default_maxvcpus = default::MAX_QEMU_VCPUS;
        }

        // 10. msize9p 填默认值（仅当使用 9p 时）
        if conf.shared_fs.shared_fs.as_deref() != Some("virtio-fs") && conf.shared_fs.msize_9p == 0
        {
            conf.shared_fs.msize_9p = default::MAX_SHARED_9PFS_SIZE_MB;
        }

        Ok(())
    }
}

#[allow(dead_code)]
impl VM {
    // pub fn pause() {
    //     info!(sl!(), "pause vm");
    //     todo!();

    //对标virt_container::lib::new_hypervisor(),这里无法访问到私有函数，而且只需要支持QEMU，所以重构了该函数。
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

    //对标virt_container::lib::new_agent(), 这里只实现了kata的
    fn new_agent(config: &VMConfig) -> Result<Arc<KataAgent>> {
        let agent_name = &config.agent_name;
        let agent_config = config.agent_config.clone();
        // .get(agent_name)
        // .ok_or_else(|| anyhow!("failed to get agent for {}", &agent_name))
        // .context("get agent")?;
        match agent_name.as_str() {
            AGENT_KATA => {
                let agent = KataAgent::new(agent_config.clone());
                Ok(Arc::new(agent))
            }
            _ => Err(anyhow!("Unsupported agent {}", &agent_name)),
        }
    }

    //创建一个空的sandbox_config结构体
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

    /// Creates a new VM based on the provided configuration.
    #[allow(unused_variables)]
    pub async fn new_vm(config: VMConfig, toml_config: TomlConfig) -> Result<Self> {
        info!(sl!(), "vm::new_vm"; "VMConfig" => format!("{:?}", config));
        //sid
        let sid = "xxx";
        //msg_sender
        const MESSAGE_BUFFER_SIZE: usize = 8;
        let (sender, _receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);

        // 这里从似乎要根据tobeTemplate来进行切换
        // hypervisor
        let hypervisor = Self::new_hypervisor(&config)
            .await
            .context("new hypervisor")?;
        // let hypervisor = crate::new_hypervisor(&toml_config).await.context("new hypervisor")?;

        //agent
        // get uds from hypervisor and get config from toml_config
        let agent = Self::new_agent(&config).context("new agent")?;
        // let agent = crate::new_agent(&toml_config).context("new agent")?;

        // sandbox_config
        let sandbox_config = Self::new_empty_sandbox_config();
        // info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sandbox_config));

        let mut initial_size_manager = InitialSizeManager::new_from(&sandbox_config.annotations)
            .context("failed to construct static resource manager")?;
        // 这里要用runtime信息更新toml_config, 暂时直接在配置文件里设置了slot和maxmemory不为0
        // initial_size_manager
        //     .setup_config(&mut toml_config)
        //     .context("failed to setup static resource mgmt config")?;

        // info!(sl!(), "vm::new_vm"; "toml_config" => format!("{:#?}", toml_config));

        let factory = toml_config.factory.clone();

        let toml_config_arc = Arc::new(toml_config);
        // 这里toml_config的所有权就给了resource_manager
        //resource_manager
        let resource_manager = Arc::new(
            ResourceManager::new(
                sid,
                agent.clone(),
                hypervisor.clone(),
                toml_config_arc,
                initial_size_manager,
            )
            .await?,
        );

        // //sandbox_config

        let sandbox = VirtSandbox::new(
            sid,
            sender.clone(),
            agent.clone(),
            hypervisor.clone(),
            resource_manager.clone(),
            sandbox_config,
            factory,
        )
        .await;

        info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sandbox));
        // 假设你已经有一个 sandbox 实例（比如从 new_vm 得到的 Ok(sandbox)）
        let sb = sandbox.unwrap(); // 如果 sandbox 是 Result<VirtSandbox, _>

        // info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sb.sid));
        // 调用 start()
        match sb.start_template().await {
            Ok(_) => {
                info!(sl!(), "vm::new_vm"; "sb" => format!("{:?}", sb));
            }
            Err(e) => {
                error!(sl!(), "sandbox start failed: {}", e);
            }
        }
        info!(sl!(), "vm::new_vm end");
        let hypervisor_config = sb.hypervisor.hypervisor_config().await;
        info!(sl!(), "vm::new_vm"; "hypervisor_config" => format!("{:?}", hypervisor_config));
        let vm = VM {
            id: sb.sid,
            hypervisor: sb.hypervisor.clone(),
            agent: sb.agent.clone(),
            cpu: hypervisor_config.cpu_info.default_vcpus,
            memory: hypervisor_config.memory_info.default_memory,
            cpu_delta: 0,
            // store,
        };
        Ok(vm)
    }


    // 把模板 VM 的 hypervisor（里面包含 agent socket 地址）复制给 sandbox，让 shim 在后续建立连接时，实际上连的就是模板 VM 内已经恢复的 agent。
    pub async fn assign_sandbox(&self, sb: &VirtSandbox) -> Result<()>{
        info!(sl!(), "vm::assign_sandbox(): assign_sandbox start");



        // 把一个 VM 和 Sandbox 绑定起来，主要做了三件事：

        // 复用 VM 内已存在的 agent；

        // 通过符号链接把 Sandbox 的共享目录、socket 指向 VM 的实际目录；

        // 更新 Sandbox 的 hypervisor 和 VM id 信息。
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!(sl!(), "vm::stop begin");

        // 结束QEMU和VirtiofsDaemon进程
        self.hypervisor
            .stop_vm()
            .await
            .map_err(|e| anyhow::anyhow!("failed to stop vm: {}", e))?;
        info!(sl!(), "vm::stop end");

        //VMTemplate todo()
        // 可能还需要移除cgroups中的控制资源，参考go中store.Destory();
        // Methods of Manager traits in rustjail are invisible, and CgroupManager.cgroup can't be serialized.
        // So it is cumbersome to manage cgroups by this field. Instead, we use cgroups-rs::cgroup directly in Container to manager cgroups.
        // Another solution is making some methods public outside rustjail and adding getter/setter for CgroupManager.cgroup.
        // Temporarily keep this field for compatibility.

        Ok(())
    }

    // 实现kata agent 与 VM 之间 gRPC 连接的断开函数
    pub async fn disconnect(&self) -> Result<()> {
        info!(sl!(), "vm::disconnect begin");
        info!(sl!(), "kill vm");
        // todo()
        // if let Err(e) = self.agent.disconnect() {
        //     error!("failed to disconnect agent: {}", e);
        // }
        Ok(())
    }

    // Pause pauses a VM.
    pub async fn pause(&self) -> Result<()> {
        info!(sl!(), "vm::pause(): start");
        self.hypervisor.pause_vm().await
    }
    // Save saves a VM to persistent disk.
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
