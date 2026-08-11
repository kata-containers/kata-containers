// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hypervisor::HYPERVISOR_NAME_CH;
use kata_sys_util::mount::umount_all;
use kata_types::config::TomlConfig;
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

fn validate_factory_config(toml_config: TomlConfig) -> Result<(TomlConfig, FactoryConfig)> {
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
    use kata_types::config::Hypervisor;

    #[cfg(all(
        feature = "cloud-hypervisor",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn factory_uses_adjusted_clh_bridge_count() {
        crate::register_hypervisor_config_plugins();
        let toml_config = TomlConfig::load(
            r#"
[hypervisor.clh]
path = "/bin/echo"
ctlpath = "/bin/echo"
kernel = "/bin/echo"
image = "/bin/echo"
firmware = ""
default_bridges = 0

[hypervisor.clh.factory]
enable_template = true
template_path = "/tmp/canonical-template"

[runtime]
hypervisor_name = "clh"
"#,
        )
        .unwrap();

        let (_, factory_config) = validate_factory_config(toml_config).unwrap();

        assert_eq!(
            factory_config
                .vm_config
                .hypervisor_config
                .device_info
                .default_bridges,
            kata_types::config::default::DEFAULT_CH_PCI_BRIDGES
        );
    }

    #[test]
    fn factory_config_does_not_apply_qemu_defaults_to_dragonball() {
        let mut toml_config = TomlConfig::default();
        toml_config.runtime.hypervisor_name = "dragonball".to_string();

        let mut hypervisor = Hypervisor::default();
        hypervisor.factory.enable_template = true;
        hypervisor.device_info.default_bridges = 0;
        toml_config
            .hypervisor
            .insert("dragonball".to_string(), hypervisor);

        let (_, factory_config) = validate_factory_config(toml_config).unwrap();

        assert_eq!(
            factory_config
                .vm_config
                .hypervisor_config
                .device_info
                .default_bridges,
            0
        );
    }
}
