// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hypervisor::HYPERVISOR_NAME_CH;
use kata_sys_util::mount::umount_all;
use kata_types::config::TomlConfig;
use resource::cpu_mem::initial_size::InitialSizeManager;
use serde::{Deserialize, Serialize};
use slog::{error, info, warn};

use crate::factory::{template::Template, vm::VmConfig};

pub mod template;
pub mod vm;

/// Returns the path to the hypervisor's device-state artifact in the template directory.
pub(crate) fn template_device_state_path(hypervisor_name: &str, template_path: &Path) -> PathBuf {
    let state_file = match hypervisor_name {
        HYPERVISOR_NAME_CH => "state.json",
        _ => "state",
    };

    template_path.join(state_file)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FactoryConfig {
    /// Path to the directory where VM templates are stored.
    #[serde(default)]
    pub template_path: String,

    /// Full configuration of the virtual machine to be used.
    #[serde(default)]
    pub vm_config: VmConfig,

    /// Whether VM template feature is enabled.
    #[serde(default)]
    pub template: bool,
}

impl FactoryConfig {
    pub fn new(toml_config: &TomlConfig) -> Self {
        Self {
            template: toml_config.get_factory().enable_template,
            template_path: toml_config.get_factory().template_path,
            vm_config: VmConfig::new(toml_config),
        }
    }
}

/// Returns an error if a sandbox VM with `config` cannot be restored from the template.
pub(crate) fn check_template_vm_config(config: &TomlConfig) -> Result<()> {
    let template_path = PathBuf::from(config.get_factory().template_path);
    let template_config = Template::load_vm_config(&template_path)?;

    if template_config != vm_config_value(&VmConfig::new(config))? {
        return Err(anyhow!("sandbox VM config does not match the template"));
    }

    Ok(())
}

/// The config used to match templates, ignoring the template paths and flags.
pub(crate) fn vm_config_value(config: &VmConfig) -> Result<serde_json::Value> {
    let mut config = config.clone();
    config.hypervisor_config.vm_template = Default::default();

    Ok(serde_json::to_value(config)?)
}

fn validate_factory_config(mut toml_config: TomlConfig) -> Result<(TomlConfig, FactoryConfig)> {
    // Give the template the CPU and memory of a sandbox without resource limits.
    InitialSizeManager::new_from(&HashMap::new())?
        .setup_config(&mut toml_config)
        .context("setup template vm size")?;

    let factory_config = FactoryConfig::new(&toml_config);

    if !factory_config.template {
        return Err(anyhow!("vm factory is not enabled"));
    }

    Ok((toml_config, factory_config))
}

pub async fn init_factory_command(toml_config: TomlConfig) -> Result<()> {
    let (toml_config, mut factory_config) = validate_factory_config(toml_config)?;

    new_factory(&mut factory_config, toml_config, false)
        .await
        .context("new factory")?;

    info!(sl!(), "create vm factory successfully");

    Ok(())
}

pub async fn destroy_factory_command(toml_config: TomlConfig) -> Result<()> {
    let (toml_config, mut factory_config) = validate_factory_config(toml_config)?;

    new_factory(&mut factory_config, toml_config, true)
        .await
        .context("new factory")?;

    close_factory(&mut factory_config).context(" close VM factory")?;

    info!(sl!(), "vm factory destroyed");
    Ok(())
}

pub async fn status_factory_command(toml_config: TomlConfig) -> Result<()> {
    let (toml_config, mut factory_config) = validate_factory_config(toml_config)?;

    if new_factory(&mut factory_config, toml_config, true)
        .await
        .is_ok()
    {
        info!(sl!(), "vm factory is on");
    } else {
        info!(sl!(), "vm factory is off");
    }

    Ok(())
}

pub async fn new_factory(
    config: &mut FactoryConfig,
    toml_config: TomlConfig,
    fetch_only: bool,
) -> Result<()> {
    if !config.template {
        anyhow::bail!("template must be enabled");
    } else {
        let path: PathBuf = config.template_path.clone().into();
        if fetch_only {
            Template::fetch(config.vm_config.clone(), path).context("fetch VM template")?;
        } else {
            Template::create(config.vm_config.clone(), toml_config, path)
                .await
                .context("initialize VM template factory")?;
        }
    }

    Ok(())
}

pub fn close_factory(config: &mut FactoryConfig) -> Result<()> {
    let state_path = Path::new(&config.template_path);

    // Check if the path exists
    if !state_path.exists() {
        warn!(
            sl!(),
            "Template path {:?} does not exist, skipping unmount", state_path
        );
        return Ok(());
    }

    // Use umount_all to unmount all filesystems at the mountpoint
    // First try normal umount (lazy_umount = false)
    if let Err(e) = umount_all(state_path, false) {
        error!(sl!(), "Normal umount failed for {:?}: {}", state_path, e);

        // If normal umount fails, try lazy umount (with MNT_DETACH flag)
        umount_all(state_path, true)
            .with_context(|| format!("Failed to lazy unmount {}", state_path.display()))?;

        info!(sl!(), "Lazy umount succeeded for {:?}", state_path);
    } else {
        info!(sl!(), "Normal umount succeeded for {:?}", state_path);
    }

    // Remove the directory after successful unmount
    fs::remove_dir_all(state_path)
        .with_context(|| format!("failed to remove {}", state_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::RuntimeHandler;
    use kata_types::annotations::cri_containerd::SANDBOX_MEM_KEY;

    fn qemu_static_config(template_path: &Path) -> TomlConfig {
        crate::VirtContainer::init().unwrap();
        TomlConfig::load(&format!(
            r#"
[hypervisor.qemu]
path = "/bin/echo"
kernel = "/bin/echo"
image = "/bin/echo"
firmware = ""

[hypervisor.qemu.factory]
enable_template = true
template_path = "{}"

[runtime]
hypervisor_name = "qemu"
static_sandbox_resource_mgmt = true
"#,
            template_path.display()
        ))
        .unwrap()
    }

    fn sandbox_config(config: &TomlConfig, annotations: HashMap<String, String>) -> TomlConfig {
        let mut config = config.clone();
        InitialSizeManager::new_from(&annotations)
            .unwrap()
            .setup_config(&mut config)
            .unwrap();
        config
    }

    #[test]
    fn template_is_used_only_when_its_saved_vm_config_matches() {
        let dir = tempfile::tempdir().unwrap();
        let config = qemu_static_config(dir.path());
        let (_, factory_config) = validate_factory_config(config.clone()).unwrap();
        let unlimited = sandbox_config(&config, HashMap::new());
        assert!(check_template_vm_config(&unlimited).is_err());

        Template::new(factory_config.vm_config, dir.path().to_path_buf())
            .save_vm_config()
            .unwrap();
        assert!(check_template_vm_config(&unlimited).is_ok());

        let limits = HashMap::from([(SANDBOX_MEM_KEY.to_string(), (512u64 << 20).to_string())]);
        let limited = sandbox_config(&config, limits);
        assert!(check_template_vm_config(&limited).is_err());
    }
}
