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
use sha2::{Digest, Sha256};
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
    pub template_path: PathBuf,

    /// Full configuration of the virtual machine to be used.
    #[serde(default)]
    pub vm_config: VmConfig,

    /// Whether VM template feature is enabled.
    #[serde(default)]
    pub template: bool,
}

impl FactoryConfig {
    fn new(toml_config: &TomlConfig) -> Result<Self> {
        let vm_config = VmConfig::new(toml_config);
        let template = vm_config.hypervisor_config.factory.enable_template;
        Ok(Self {
            template,
            template_path: template_path(&vm_config)?,
            vm_config,
        })
    }
}

fn template_path(config: &VmConfig) -> Result<PathBuf> {
    let factory = &config.hypervisor_config.factory;
    let Some(pool) = &factory.template_pool else {
        return Ok(PathBuf::from(&factory.template_path));
    };
    let pool = Path::new(pool);
    if !pool.is_absolute() {
        return Err(anyhow!("template_pool must be an absolute directory"));
    }

    let mut config = vm_config_value(config)?;
    config.sort_all_objects();
    let digest = Sha256::digest(serde_json::to_vec(&config)?);
    Ok(pool.join(format!("vm-{}", hex::encode(digest))))
}

/// Use a template only when its snapshot files and saved configuration match.
pub(crate) fn matching_template_path(config: &TomlConfig) -> Result<PathBuf> {
    let vm_config = VmConfig::new(config);
    let path = template_path(&vm_config)?;
    let expected = vm_config_value(&vm_config)?;
    let template = Template::fetch(vm_config, path)?;
    if Template::load_vm_config(&template.state_path)? != expected {
        return Err(anyhow!("sandbox VM config does not match the template"));
    }

    Ok(template.state_path)
}

/// The config used to match templates, ignoring the template paths and flags.
pub(crate) fn vm_config_value(config: &VmConfig) -> Result<serde_json::Value> {
    let mut config = config.clone();
    config.hypervisor_config.vm_template = Default::default();
    if config.hypervisor_config.factory.template_pool.is_some() {
        // Where snapshots are stored does not affect the guest configuration.
        config.hypervisor_config.factory = Default::default();
    }

    Ok(serde_json::to_value(config)?)
}

fn validate_factory_config(mut toml_config: TomlConfig) -> Result<(TomlConfig, FactoryConfig)> {
    // Give the template the CPU and memory of a sandbox without resource limits.
    InitialSizeManager::new_from(&HashMap::new())?
        .setup_config(&mut toml_config)
        .context("setup template vm size")?;

    let factory_config = FactoryConfig::new(&toml_config)?;

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
        let path = config.template_path.clone();
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
    let state_path = &config.template_path;

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

    fn save_fixture(config: &FactoryConfig) {
        fs::create_dir_all(&config.template_path).unwrap();
        fs::write(config.template_path.join("memory"), b"memory").unwrap();
        fs::write(config.template_path.join("state"), b"state").unwrap();
        Template::new(config.vm_config.clone(), config.template_path.clone())
            .save_vm_config()
            .unwrap();
    }

    #[test]
    fn template_is_used_only_when_its_saved_vm_config_matches() {
        let dir = tempfile::tempdir().unwrap();
        let config = qemu_static_config(dir.path());
        let (_, factory_config) = validate_factory_config(config.clone()).unwrap();
        let unlimited = sandbox_config(&config, HashMap::new());
        assert!(matching_template_path(&unlimited).is_err());

        save_fixture(&factory_config);
        assert_eq!(matching_template_path(&unlimited).unwrap(), dir.path());

        let limits = HashMap::from([(SANDBOX_MEM_KEY.to_string(), (512u64 << 20).to_string())]);
        let limited = sandbox_config(&config, limits);
        assert!(matching_template_path(&limited).is_err());
    }

    #[test]
    fn template_pool_selects_the_effective_sandbox_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = qemu_static_config(&dir.path().join("legacy"));
        let hypervisor = config.hypervisor.get_mut("qemu").unwrap();
        hypervisor.factory.template_pool = Some(dir.path().join("pool").display().to_string());
        let (_, default) = validate_factory_config(config.clone()).unwrap();
        save_fixture(&default);

        let limits =
            |mib: u64| HashMap::from([(SANDBOX_MEM_KEY.to_string(), (mib << 20).to_string())]);
        let large = sandbox_config(&config, limits(768));
        let unavailable = sandbox_config(&config, limits(1024));
        assert!(matching_template_path(&large).is_err());

        // Factory init for the larger spec uses effective defaults rather than
        // the annotations that a real sandbox receives from the caller.
        let mut profile = large.clone();
        profile
            .hypervisor
            .get_mut("qemu")
            .unwrap()
            .factory
            .template_path = "/unused".into();
        let (_, large_template) = validate_factory_config(profile).unwrap();
        save_fixture(&large_template);

        let default_vm = sandbox_config(&config, HashMap::new());
        assert_ne!(default.template_path, large_template.template_path);
        assert_eq!(
            matching_template_path(&default_vm).unwrap(),
            default.template_path
        );
        assert_eq!(
            matching_template_path(&large).unwrap(),
            large_template.template_path
        );
        assert!(matching_template_path(&unavailable).is_err());

        fs::remove_file(large_template.template_path.join("state")).unwrap();
        assert!(matching_template_path(&large).is_err());
        assert!(matching_template_path(&default_vm).is_ok());
    }
}
