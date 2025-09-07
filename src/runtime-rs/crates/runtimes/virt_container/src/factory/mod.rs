#![allow(unused_variables, unused_imports)]
use anyhow::Result;
use anyhow::Context;
// use hypervisor::factory::factory;

use slog::{info, error};

use serde::{Deserialize, Serialize};

use kata_types::config::{TomlConfig};

use std::path::PathBuf;

use crate::factory::template::Template;
use crate::factory::vm::{VMConfig, VM};

pub mod template;
pub mod vm;

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory"))
    };
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

pub async fn init_factory_command() -> Result<()> {
    info!(sl!(), "init_factory_command");
    // Step 1: Load Kata config
    let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

    // Step 2: Build FactoryConfig
    let mut factory_config = FactoryConfig {
        template: toml_config.factory.template,
        template_path: toml_config.factory.template_path.clone(),
        cache: toml_config.factory.vm_cache_number,
        vm_cache: toml_config.factory.vm_cache_number > 0,
        vm_config: VMConfig {
            hypervisor_name: toml_config.runtime.hypervisor_name.clone(),
            agent_name: toml_config.runtime.agent_name.clone(),
            hypervisor_config: toml_config.hypervisor.get(&toml_config.runtime.hypervisor_name)
                .cloned()
                .unwrap_or_default(),
            agent_config: toml_config.agent.get(&toml_config.runtime.agent_name)
                .cloned()
                .unwrap_or_default(),
        },
    };

    // Template
    if toml_config.factory.template {
        // info!(sl!(), "create vm factory"; "factory_config" => format!("{:?}", factory_config));
        info!(sl!(), "create vm factory");

        match new_factory(&mut factory_config, toml_config, false).await {
            Ok(_) => {
            }
            Err(e) => {
                error!(sl!(), "create vm factory failed: {}", e);
                return Err(e);
            }
        }
    } else {
            let err_string = "vm factory or VMCache is not enabled";
            error!(sl!(), "{}", err_string);
    }

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


pub async fn new_factory(config: &mut FactoryConfig, toml_config: TomlConfig, fetch_only: bool) -> Result<()> {
    // 1. 校验 VMConfig
    if let Err(e) = config.vm_config.valid() {
        error!(sl!(), "{:#?}", e);
        return Err(e);
    }
    else {
        info!(sl!(), "VMConfig validate ok");
    }

    // 2. 仅支持 template 模式
    if !config.template {
        error!(sl!(), "template must be enabled");
    }
    else {
        if fetch_only {
            info!(sl!(), "template.Fetch");
            // 构造 PathBuf
            let path: PathBuf = config.template_path.clone().into();
            let factory = Template::fetch(config.vm_config.clone(), path);
            // info!(sl!(), "{:?}", factory);
        }
        else {
            info!(sl!(),"template.New");
            // 构造 PathBuf
            let path: PathBuf = config.template_path.clone().into();
            let factory = match Template::new(config.vm_config.clone(), toml_config, path).await {
                Ok(factory) => factory,
                Err(e) => {
                    error!(sl!(), "Failed to create new Template factory: {}", e);
                    return Err(e);
                }
            };
            // info!(sl!(), "Created factory: {:?}", factory);
        }
    }

    Ok(())
}

pub async fn get_vm(config: &mut VMConfig, template_path: PathBuf) -> Result<VM>
{
    
    info!(sl!(), "factory::get_vm(): start");

    // 1. 校验 VMConfig
    if let Err(e) = config.valid() {
        error!(sl!(), "{:#?}", e);
        return Err(e);
    }
    else {
        info!(sl!(), "factory::get_vm(): VMConfig validate ok");
    }

    //2. 比较VM模版与新VM的模版是否匹配，如果不匹配则还是直接启动VM
    //todo 
    //vm.checkVMConfig() if true template; else direct

    //3. 获取templateVM
    //go里通过 base接口来实现多种VM（cache、template等）的获取，这里暂时只实现template
    let template = Template {
        state_path: template_path,
        config: config.clone(),
    };
    let vm = template.get_base_vm(config).await?;
    info!(sl!(),"factory::get_vm(): vm: new_vm() VM id={}, cpu={}, memory={}", vm.id, vm.cpu, vm.memory);
    // 4.恢复被paused的vm
    vm.resume().await?;

    //5.重新为 guest 内核的 RNG 设备注入新种子，保证 clone 出来的 VM 仍然有不同的随机性
    // todo vm.ReseedRNG()

    //6.把 guest 内部时钟同步到 宿主机当前时间，保证 VM 的时间准确
    //todo vm.SyncTime()

    //7.factory 恢复 VM 后，对 CPU 和内存进行“热扩展 (hotplug)”的处理
    //tofo vm.AddCPUs(); vm.AddMemory; vm.OnlineCPUMemory

    //8.返回vm
    Ok(vm)

//     检查配置

// 验证传入的 VMConfig 是否有效。

// 如果配置非法，直接返回错误。

// 选择路径

// 调用 checkConfig 检查工厂是否支持该配置。

// 不支持时 → 回退到 direct factory，直接新建 VM。

// 获取模版 VM

// 调用 base.GetBaseVM() 从工厂模版中获取一台 VM 副本。从template调用createFromTemplateVM实现

// 如果失败，返回错误。

// 错误清理机制

// 注册 defer，如果后续出错则调用 vm.Stop() 清理资源。

// 恢复 VM 状态

// vm.Resume()：恢复暂停的 VM。

// vm.ReseedRNG()：重新播种随机数，避免 VM 间随机数冲突。

// vm.SyncTime()：同步 guest 时间。

// 动态扩展资源

// 比较模版 VM 与目标配置：

// 如果 CPU 不足 → 调用 vm.AddCPUs() 扩展。

// 如果内存不足 → 调用 vm.AddMemory() 扩展。

// 调用 vm.OnlineCPUMemory() 让扩展的资源上线。

// 返回可用 VM

// 返回一台 恢复完成 + 满足需求 的 VM，供 Sandbox 使用。
}