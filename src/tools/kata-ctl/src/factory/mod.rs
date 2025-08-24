#![allow(unused_variables, unused_imports)]
use anyhow::Result;
use anyhow::Context;
// use hypervisor::factory::factory;

use slog::{info, error};

use serde::{Deserialize, Serialize};

use kata_types::config::{TomlConfig};

use std::path::PathBuf;

use crate::factory::template::Template;
use crate::factory::vm::{VMConfig};

mod template;
mod vm;

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
