// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use slog::{error, info};

use kata_types::config::TomlConfig;

use crate::factory::template::Template;
use crate::factory::vm::{VMConfig, VM};
use hypervisor::firecracker::sl;

pub mod template;
pub mod vm;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FactoryConfig {
    /// Path to the directory where VM templates are stored.
    #[serde(default)]
    pub template_path: String,

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
    // Load Kata config
    let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

    // Build FactoryConfig
    let mut factory_config = FactoryConfig {
        template: toml_config.factory.template,
        template_path: toml_config.factory.template_path.clone(),
        cache: toml_config.factory.vm_cache_number,
        vm_cache: toml_config.factory.vm_cache_number > 0,
        vm_config: VMConfig {
            hypervisor_name: toml_config.runtime.hypervisor_name.clone(),
            agent_name: toml_config.runtime.agent_name.clone(),
            hypervisor_config: toml_config
                .hypervisor
                .get(&toml_config.runtime.hypervisor_name)
                .cloned()
                .unwrap_or_default(),
            agent_config: toml_config
                .agent
                .get(&toml_config.runtime.agent_name)
                .cloned()
                .unwrap_or_default(),
        },
    };

    // Template
    if !toml_config.factory.template {
        return Err(anyhow!("vm factory or VMCache is not enabled"));
    }

    new_factory(&mut factory_config, toml_config, false)
        .await
        .context("factory::init_factory_command(): create vm factory failed")?;

    info!(
        sl!(),
        "factory::init_factory_command(): create vm factory successfully"
    );

    Ok(())
}

pub async fn destroy_factory_command() -> Result<()> {
    println!("destroy_factory_command");
    // Load Kata config
    let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

    // Build FactoryConfig
    let mut factory_config = FactoryConfig {
        template: toml_config.factory.template,
        template_path: toml_config.factory.template_path.clone(),
        cache: toml_config.factory.vm_cache_number,
        vm_cache: toml_config.factory.vm_cache_number > 0,
        vm_config: VMConfig {
            hypervisor_name: toml_config.runtime.hypervisor_name.clone(),
            agent_name: toml_config.runtime.agent_name.clone(),
            hypervisor_config: toml_config
                .hypervisor
                .get(&toml_config.runtime.hypervisor_name)
                .cloned()
                .unwrap_or_default(),
            agent_config: toml_config
                .agent
                .get(&toml_config.runtime.agent_name)
                .cloned()
                .unwrap_or_default(),
        },
    };

    // Template
    if toml_config.factory.template {
        new_factory(&mut factory_config, toml_config, true)
            .await
            .map_err(|e| {
                error!(sl!(), "load vm factory failed: {:?}", e);
                anyhow!(e).context("failed to load VM factory")
            })?;

        info!(sl!(), "begin destroy factory");

        close_factory(&mut factory_config).map_err(|e| {
            error!(sl!(), "Failed to close factory: {:?}", e);
            anyhow!("Failed to close factory: {}", e)
        })?;
    } else {
        info!(sl!(), "vm factory is not enabled");
    }

    info!(sl!(), "vm factory destroyed");
    Ok(())
}

pub async fn status_factory_command() -> Result<()> {
    println!("status_factory_command");

    // Load Kata config
    let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

    // Build FactoryConfig
    let mut factory_config = FactoryConfig {
        template: toml_config.factory.template,
        template_path: toml_config.factory.template_path.clone(),
        cache: toml_config.factory.vm_cache_number,
        vm_cache: toml_config.factory.vm_cache_number > 0,
        vm_config: VMConfig {
            hypervisor_name: toml_config.runtime.hypervisor_name.clone(),
            agent_name: toml_config.runtime.agent_name.clone(),
            hypervisor_config: toml_config
                .hypervisor
                .get(&toml_config.runtime.hypervisor_name)
                .cloned()
                .unwrap_or_default(),
            agent_config: toml_config
                .agent
                .get(&toml_config.runtime.agent_name)
                .cloned()
                .unwrap_or_default(),
        },
    };

    // Template
    if toml_config.factory.template {
        if new_factory(&mut factory_config, toml_config, true)
            .await
            .is_ok()
        {
            info!(sl!(), "vm factory is on");
        } else {
            info!(sl!(), "vm factory is off");
        }
    } else {
        info!(sl!(), "vm factory is not enabled");
    }

    Ok(())
}

pub async fn new_factory(
    config: &mut FactoryConfig,
    toml_config: TomlConfig,
    fetch_only: bool,
) -> Result<()> {
    // Validate VMConfig
    config.vm_config.valid().map_err(|e| {
        error!(
            sl!(),
            "factory::new_factory(): VMConfig validate failed {:#?}", e
        );
        e
    })?;

    info!(sl!(), "factory::new_factory(): VMConfig validate ok");

    // template mode
    if !config.template {
        error!(sl!(), "factory::new_factory(): template must be enabled");
    } else {
        if fetch_only {
            info!(sl!(), "factory::new_factory(): template fetch");
            // Construct PathBuf
            let path: PathBuf = config.template_path.clone().into();
            Template::fetch(config.vm_config.clone(), path)
                .context("failed to fetch VM template")?;
        } else {
            info!(sl!(), "factory::new_factory(): template new");
            // Construct PathBuf
            let path: PathBuf = config.template_path.clone().into();
            Template::new(config.vm_config.clone(), toml_config, path)
                .await
                .inspect_err(|e| error!(sl!(), "Failed to create new Template factory: {}", e))
                .context("failed to initialize VM template factory")?;
        }
    }

    Ok(())
}

pub fn close_factory(config: &mut FactoryConfig) -> Result<()> {
    let state_path = Path::new(&config.template_path);

    let c_state_path = CString::new(
        state_path
            .to_str()
            .context("template_path is not valid UTF-8")?,
    )
    .context("template_path contains null byte")?;

    let result = unsafe { libc::umount(c_state_path.as_ptr()) };

    if result != 0 {
        // if umount false，try to use umount -f to force umount
        let result_lazy = Command::new("umount")
            .arg("-f")
            .arg(state_path)
            .output()
            .context("Failed to execute umount command with -f")?;

        if !result_lazy.status.success() {
            let err = std::io::Error::last_os_error();
            let msg = format!("failed to force unmount {}: {}", state_path.display(), err);
            error!(sl!(), "{}", msg);
            return Err(anyhow!(msg));
        }
    }

    // Attempt to remove the directory and its contents
    fs::remove_dir_all(&state_path)
        .inspect_err(|e| error!(sl(), "Failed to remove {}: {}", state_path.display(), e))
        .context(format!("Failed to remove {}", state_path.display()))?;
    Ok(())
}

pub async fn get_vm(config: &mut VMConfig, template_path: PathBuf) -> Result<VM> {
    info!(sl!(), "factory::get_vm(): start");

    // Validate  VMConfig
    config.valid().inspect_err(|e| error!(sl!(), "{:#?}", e))?;
    info!(sl!(), "factory::get_vm(): VMConfig validate ok");

    // Get template
    let template = Template {
        state_path: template_path,
        config: config.clone(),
    };

    let vm = template.get_base_vm(config).await?;
    info!(
        sl!(),
        "factory::get_vm(): vm: new_vm() VM id={}, cpu={}, memory={}", vm.id, vm.cpu, vm.memory
    );

    // Resume vm
    vm.resume().await?;
    Ok(vm)
}
