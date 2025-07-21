// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(unused_variables, unused_imports)]
use anyhow::{anyhow, Context, Result};
use containerd_shim_protos::ttrpc::asynchronous::shutdown::new;
use hypervisor::firecracker::sl;
use slog::{error, info};

use serde::{Deserialize, Serialize};

use kata_types::config::TomlConfig;

use std::ffi::CString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::factory::template::Template;
use crate::factory::vm::{VMConfig, VM};

pub mod template;
pub mod vm;

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory"))
    };
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
        match new_factory(&mut factory_config, toml_config, false).await {
            Ok(_) => {
                info!(
                    sl!(),
                    "factory::init_factory_command(): create vm factory successfully"
                );
            }
            Err(e) => {
                error!(
                    sl!(),
                    "factory::init_factory_command(): create vm factory failed: {}", e
                );
                return Err(e);
            }
        }
    } else {
        let err_string = "vm factory or VMCache is not enabled";
        error!(sl!(), "{}", err_string);
    }

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
        if let Err(e) = new_factory(&mut factory_config, toml_config, true).await {
            error!(sl!(), "load vm factory failed: {:?}", e);
            return Err(e);
        }

        info!(sl!(), "begin destroy factory");

        close_factory(&mut factory_config).map_err(|e| {
            error!(sl!(), "Failed to close factory: {:?}", e);
            anyhow!("Failed to close factory: {}", e)
        })?;
    } else {
        let log_string = "vm factory is not enabled";
        info!(sl!(), "{}", log_string);
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
        match new_factory(&mut factory_config, toml_config, true).await {
            Ok(_) => {
                info!(sl!(), "vm factory is on");
            }
            Err(e) => {
                info!(sl!(), "vm factory is off");
            }
        }
    } else {
        let log_string = "vm factory is not enabled";
        info!(sl!(), "{}", log_string);
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
            let factory = match Template::fetch(config.vm_config.clone(), path) {
                Ok(factory) => {
                    // Successfully fetched the factory, proceed with the logic
                    factory
                }
                Err(e) => {
                    return Err(anyhow!(e)); // Return an error with a detailed message
                }
            };
        } else {
            info!(sl!(), "factory::new_factory(): template new");
            // Construct PathBuf
            let path: PathBuf = config.template_path.clone().into();
            let factory = match Template::new(config.vm_config.clone(), toml_config, path).await {
                Ok(factory) => factory,
                Err(e) => {
                    error!(sl!(), "Failed to create new Template factory: {}", e);
                    return Err(e);
                }
            };
        }
    }

    Ok(())
}

pub fn close_factory(config: &mut FactoryConfig) -> Result<()> {
    let state_path = Path::new(&config.template_path); // Get the state path from the config
    let c_state_path = CString::new(state_path.to_str().unwrap())?;
    // Attempt to unmount the state path
    let result = unsafe { libc::umount(c_state_path.as_ptr()) }; // Unmount the directory

    if result != 0 {
        // if umount false，try to use umount -f to force umount
        let result_lazy = Command::new("umount")
            .arg("-f")
            .arg(state_path)
            .output()
            .context("Failed to execute umount command with -f")?;

        if !result_lazy.status.success() {
            let err = std::io::Error::last_os_error();
            error!(
                sl!(),
                "Failed to force unmount {}: {}",
                state_path.display(),
                err
            );
            return Err(anyhow!(
                "Failed to force unmount {}: {}",
                state_path.display(),
                err
            ));
        }
    }

    // Attempt to remove the directory and its contents
    if let Err(e) = fs::remove_dir_all(&state_path) {
        // Log an error if removing the directory fails
        error!(sl(), "Failed to remove {}: {}", state_path.display(), e);
        return Err(anyhow!("Failed to remove {}: {}", state_path.display(), e));
    }

    Ok(())
}

pub async fn get_vm(config: &mut VMConfig, template_path: PathBuf) -> Result<VM> {
    info!(sl!(), "factory::get_vm(): start");

    // Validate  VMConfig
    if let Err(e) = config.valid() {
        error!(sl!(), "{:#?}", e);
        return Err(e);
    } else {
        info!(sl!(), "factory::get_vm(): VMConfig validate ok");
    }

    // Compare the VM template with the new VM template.
    // If they do not match, directly start the VM instead
    //todo vm.checkVMConfig() if true template; else direct

    // Get template
    // In Go, multiple VM types (cache, template, etc.) are accessed through the base interface.
    // Here, we are currently implementing only the template.
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

    // Re-inject a new seed into the guest kernel's RNG device to ensure that the cloned VM
    // todo vm.ReseedRNG()

    // Synchronize the guest internal clock with the host's current time to ensure the VM's time is accurate.
    // todo vm.SyncTime()

    //  After restoring the VM in the factory, perform "hotplug" processing for CPU and memory expansion.
    // tofo vm.AddCPUs(); vm.AddMemory; vm.OnlineCPUMemory

    Ok(vm)
}
