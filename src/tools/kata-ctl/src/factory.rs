use anyhow::Result;
use anyhow::Context;

use slog::{info};

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory"))
    };
}

use serde::{Deserialize, Serialize};

use kata_types::config::{TomlConfig, Agent, Hypervisor};

/// VMConfig holds all configuration information required to start a new VM instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VMConfig {
    /// Type of hypervisor to be used (e.g., qemu, cloud-hypervisor).
    #[serde(default)]
    pub hypervisor_name: String,

    /// Configuration for the guest agent.
    #[serde(default)]
    pub agent_config: Agent,

    /// Configuration for the hypervisor.
    #[serde(default)]
    pub hypervisor_config: Hypervisor,
}

#[derive(Debug, Clone, Default,Serialize, Deserialize)]
pub struct FactoryConfig {
    /// Path to the directory where VM templates are stored.
    #[serde(default)]
    pub template_path: String,

    /// Endpoint used for communication with the VM cache server.
    // #[serde(default)]
    // pub vm_cache_endpoint: String,

    /// Full configuration of the virtual machine to be used.
    #[serde(default)]
    pub vm_config: VMConfig,

    /// Number of cached VMs to maintain.
    #[serde(default)]
    pub cache: u32,

    /// Whether VM template feature is enabled.
    #[serde(default)]
    pub template: bool,

    /// Whether VM cache feature is enabled.
    #[serde(default)]
    pub vm_cache: bool,
}

pub fn init_factory_command() -> Result<()> {
    info!(sl!(), "init_factory_command");
    // Step 1: load Kata config
    let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

    let factory_config = FactoryConfig {
        template: toml_config.factory.template,
        template_path: toml_config.factory.template_path.clone(),
        cache: toml_config.factory.vm_cache_number,
        vm_cache: toml_config.factory.vm_cache_number > 0,
        vm_config: VMConfig {
            hypervisor_name: toml_config.runtime.hypervisor_name.clone(),
            hypervisor_config: toml_config.hypervisor.get(&toml_config.runtime.hypervisor_name)
                .cloned()
                .unwrap_or_default(),
            agent_config: toml_config.agent.get(&toml_config.runtime.agent_name)
                .cloned()
                .unwrap_or_default(),
        },
    };
    info!(sl!(), "factory_config: {:?}",factory_config);

    // // Step 2: 构造 FactoryConfig（等价于 vf.Config）
    // let factory_config = FactoryConfig {
    //     template: runtime_config.factory_config.template,
    //     template_path: runtime_config.factory_config.template_path.clone(),
    //     cache: runtime_config.factory_config.vm_cache_number,
    //     vm_cache: runtime_config.factory_config.vm_cache_number > 0,
    //     vm_cache_endpoint: runtime_config.factory_config.vm_cache_endpoint.clone(),
    //     vm_config: runtime_config.to_vm_config(), // 假设你有方法转为 VMConfig
    // };

    // // Step 3: 如果启用了 template 模式，就创建 factory
    // if factory_config.template {
    //     log::info!("Creating VM factory with config: {:?}", factory_config);

    //     let factory = TemplateFactory::new(factory_config.clone(), runtime_config.clone()).await?;
    //     factory.create_template().await?;

    //     println!("vm factory initialized");
    // } else {
    //     // 如果 template 与 cache 都没启用
    //     let msg = "vm factory or VMCache is not enabled";
    //     log::error!("{}", msg);
    //     println!("{}", msg);
    // }
    Ok(())
}

pub fn destroy_factory_command() -> Result<()> {
    println!("destroy_factory_command");
    Ok(())
}

pub fn status_factory_command() -> Result<()> {
    println!("status_factory_command");
    Ok(())
}
