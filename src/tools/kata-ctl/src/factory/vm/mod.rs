#![allow(unused_mut)]
use anyhow::Result;
use anyhow::anyhow;
use anyhow::Context;

use std::sync::Arc;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use kata_types::config::default;
use kata_types::config::Agent as AgentConfig;
use kata_types::config::Hypervisor as HypervisorConfig;
use kata_types::config::{TomlConfig};

use hypervisor::{qemu::Qemu, HYPERVISOR_QEMU, Hypervisor};
use agent::{Agent, kata::KataAgent, AGENT_KATA};
use resource::{
    ResourceManager, 
    cpu_mem::initial_size::InitialSizeManager
};

use runtime_spec;
use virt_container::{sandbox::VirtSandbox};
use tokio::sync::mpsc::channel;
use common::{message::Message, types::SandboxConfig, SandboxNetworkEnv, Sandbox};
use slog::{error};
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "vm"))
    };
}

/// VM is an abstraction of a virtual machine.
#[allow(dead_code)]
pub struct VM {
    /// The hypervisor responsible for managing the virtual machine lifecycle.
    pub hypervisor: Box<dyn Hypervisor>,

    /// The guest agent that communicates with the virtual machine.
    pub agent: Box<dyn Agent>,

    /// Persistent storage driver to save and restore VM state.
    // pub store: Box<dyn PersistDriver>,

    /// Unique identifier of the virtual machine.
    pub id: String,

    /// Number of vCPUs assigned to the VM.
    pub cpu: u32,

    /// Amount of memory (in MB) assigned to the VM.
    pub memory: u32,

    /// Tracks the difference in vCPU count since last update.
    pub cpu_delta: u32,
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
            && conf.machine_info.machine_type == "s390-ccw-virtio" // QemuCCWVirtio
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
        if conf.cpu_info.default_vcpus == 0 {
            conf.cpu_info.default_vcpus = default::DEFAULT_GUEST_VCPUS as i32;
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
        if conf.cpu_info.default_maxvcpus == 0 || conf.cpu_info.default_maxvcpus > default::MAX_QEMU_VCPUS {
            conf.cpu_info.default_maxvcpus = default::MAX_QEMU_VCPUS;
        }

        // 10. msize9p 填默认值（仅当使用 9p 时）
        if conf.shared_fs.shared_fs.as_deref() != Some("virtio-fs") && conf.shared_fs.msize_9p == 0 {
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
                h.set_hypervisor_config(config.hypervisor_config.clone()).await;
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

    // pub struct SandboxNetworkEnv {
    //     pub netns: Option<String>,
    //     pub network_created: bool,
    // }


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
        // //hypervisor
        let hypervisor = Self::new_hypervisor(&config).await.context("new hypervisor")?;
        // let hypervisor = new_hypervisor(&config).await.context("new hypervisor")?;
        
        // //agent
        // // get uds from hypervisor and get config from toml_config
        let agent = Self::new_agent(&config).context("new agent")?;
        
        let sandbox_config = Self::new_empty_sandbox_config();
        info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sandbox_config));

        let mut initial_size_manager = InitialSizeManager::new_from(&sandbox_config.annotations)
                .context("failed to construct static resource manager")?;
        // info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sandbox_config));

        // 这里要用runtime信息更新toml_config, 暂时直接在配置文件里设置了slot和maxmemory不为0
        // initial_size_manager
        //     .setup_config(&mut toml_config)
        //     .context("failed to setup static resource mgmt config")?;
        
        
        info!(sl!(), "vm::new_vm"; "toml_config" => format!("{:#?}", toml_config));

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
        )
        .await;


        info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sandbox));
        // 假设你已经有一个 sandbox 实例（比如从 new_vm 得到的 Ok(sandbox)）
        let sb = sandbox.unwrap();  // 如果 sandbox 是 Result<VirtSandbox, _>

        // 调用 start()
        match sb.start().await {
            Ok(_) => {
                info!(sl!(), "vm::new_vm"; "sandbox" => format!("{:?}", sb));
            }
            Err(e) => {
                error!(sl!(), "sandbox start failed: {}", e);
            }
        }

        info!(sl!(), "vm::new_vm end" );
        // 1. 设置 hypervisor
        // let hypervisor = Self::new_hypervisor(&config).await?;
        //2. 设置网络
        

        todo!();

        // Ok(())



    }
}

// NewVM 函数任务清单
// （1）初始化与检查

// 创建 Hypervisor 实例：调用 NewHypervisor(config.HypervisorType)。 √

// 创建 Network 实例：调用 NewNetwork()。

// 验证配置：调用 config.Valid() 检查VM的配置有效性。 

// 生成唯一 VM ID：通过 uuid.Generate().String() 生成。 对应rs中的prepare_vm √

// 获取持久化存储驱动：调用 persist.GetDriver()。

// （2）错误处理机制

// 失败时回收资源：

// 若任意步骤报错：记录日志 (virtLog)。

// 销毁存储：store.Destroy(id)。

// （3）创建虚拟机

// 调用 Hypervisor 创建 VM：

// 使用 hypervisor.CreateVM(ctx, id, network, &config.HypervisorConfig) 创建虚拟机。

// （4）设置 Agent

// 获取 Agent 构造函数：getNewAgentFunc(ctx)。

// 初始化 Agent：agent.configure(...)，配置 agent 与 hypervisor、共享目录和 agent 配置。

// 设置 Agent URL：agent.setAgentURL()。

// （5）启动虚拟机

// 启动 VM：调用 hypervisor.StartVM(ctx, VmStartTimeout)。

// 错误时清理：若启动失败，则调用 hypervisor.StopVM(ctx, false) 关闭。

// （6）检查 Agent 状态

// 非模板启动时检查存活：

// 如果 !config.HypervisorConfig.BootFromTemplate，则执行 agent.check(ctx)。

// （7）返回 VM 实例

// 构建并返回 VM 对象：

// 包含 id, hypervisor, agent, cpu, memory, store 等字段。