// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::Debug;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use hypervisor::HYPERVISOR_NAME_CH;
use kata_sys_util::mount::{get_linux_mount_info, Error as MountError};
use kata_types::config::TomlConfig;
use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use slog::warn;
use tokio::sync::mpsc::channel;
use tokio::time::sleep;

use crate::factory::{
    template_device_state_path,
    vm::{TemplateVm, VmConfig},
    vm_config_value,
};

/// Maximum time to wait for the Kata Agent to become ready when initializing a template VM.
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);
const MESSAGE_BUFFER_SIZE: usize = 8;

/// Preallocated size (in MB) for saving the device state snapshot of the template VM.
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8;
const MIB: u64 = 1024 * 1024;

/// Cloud Hypervisor already writes its own `config.json` here.
const TEMPLATE_VM_CONFIG_FILE: &str = "vm_config.json";

pub(crate) fn lock_template_parent(path: &Path) -> Result<Flock<File>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = File::open(parent).with_context(|| format!("open {}", parent.display()))?;
    Flock::lock(dir, FlockArg::LockExclusiveNonblock).map_err(|(_, err)| {
        let message = if err == Errno::EWOULDBLOCK {
            format!("template directory {} is busy", parent.display())
        } else {
            format!("lock {}", parent.display())
        };
        anyhow::Error::new(err).context(message)
    })
}

enum MountState {
    Unmounted,
    Mounted,
    Retained,
}

struct TemplateMount<'a> {
    path: &'a Path,
    created_dir: bool,
    state: MountState,
}

impl<'a> TemplateMount<'a> {
    fn prepare(path: &'a Path, memory_mib: u32) -> Result<Self> {
        let created_dir = match fs::create_dir(path) {
            Ok(()) => true,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => false,
            Err(err) => return Err(err).with_context(|| format!("create {}", path.display())),
        };
        let mut prepared = Self {
            path,
            created_dir,
            state: MountState::Unmounted,
        };
        let mount_path = path.to_str().context("template path is not UTF-8")?;
        match get_linux_mount_info(mount_path) {
            Err(MountError::NoMountEntry(_)) => (),
            Err(err) => return Err(err).context("check template mount point"),
            Ok(_) => {
                return Err(anyhow!(
                    "template path {} is already mounted",
                    path.display()
                ))
            }
        }
        if !fs::symlink_metadata(path)?.is_dir() || fs::read_dir(path)?.next().is_some() {
            return Err(anyhow!(
                "template path {} must be an empty directory",
                path.display()
            ));
        }

        // Keep the guest's memory and its device state on tmpfs.
        let size_mib = u64::from(memory_mib) + u64::from(TEMPLATE_DEVICE_STATE_SIZE_MB);
        mount(
            Some("tmpfs"),
            path,
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some(format!("size={size_mib}M").as_str()),
        )
        .with_context(|| format!("mount tmpfs at {}", path.display()))?;
        prepared.state = MountState::Mounted;
        Ok(prepared)
    }

    fn rollback(&mut self) -> Result<()> {
        match self.state {
            MountState::Retained => return Ok(()),
            MountState::Mounted => {
                umount2(self.path, MntFlags::MNT_DETACH | MntFlags::UMOUNT_NOFOLLOW)
                    .with_context(|| format!("unmount incomplete template {:?}", self.path))?;
                self.state = MountState::Unmounted;
            }
            MountState::Unmounted => {}
        }
        if self.created_dir {
            fs::remove_dir(self.path)
                .with_context(|| format!("remove template directory {:?}", self.path))?;
            self.created_dir = false;
        }
        Ok(())
    }
}

impl Drop for TemplateMount<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.rollback() {
            warn!(sl!(), "failed to roll back template mount: {err:#}");
        }
    }
}

#[derive(Debug)]
pub struct Template {
    pub state_path: PathBuf,
    pub config: VmConfig,
}

impl Template {
    /// Creates a new Template instance with the given configuration and path.
    pub fn new(config: VmConfig, template_path: PathBuf) -> Self {
        Template {
            state_path: template_path,
            config,
        }
    }

    pub fn fetch(config: VmConfig, template_path: PathBuf) -> Result<Box<Template>> {
        let t = Template::new(config, template_path);

        // Call template_vm_exists to validate the template's files
        if !t.template_vm_exists() {
            return Err(anyhow!("no template vm found"));
        }

        Ok(Box::new(t))
    }

    /// Creates and saves a new template VM to disk.
    /// This will prepare template files, create a VM, and save its state.
    pub async fn create(
        config: VmConfig,
        toml_config: TomlConfig,
        template_path: PathBuf,
    ) -> Result<Box<Template>> {
        let t = Template::new(config, template_path);

        if let Some(parent) = t.state_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).context("create template parent directory")?;
        }
        let _lock = lock_template_parent(&t.state_path)?;
        if t.template_vm_exists() {
            return Err(anyhow!(
                "There is already a VM template in {:?}",
                t.state_path
            ));
        }

        let memory_mib = t.config.hypervisor_config.memory_info.default_memory;
        let mut tmpfs = TemplateMount::prepare(&t.state_path, memory_mib)?;
        let result: Result<()> = async {
            let memory_file = t.memory_path();
            File::create(&memory_file)
                .and_then(|file| file.set_len(u64::from(memory_mib) * MIB))
                .with_context(|| format!("prepare template memory file {memory_file:?}"))?;
            t.save_to_template(toml_config, &mut tmpfs)
                .await
                .context("create template files")?;
            t.save_vm_config().context("save template vm config")
        }
        .await;
        if let Err(err) = result {
            if let Err(rollback_err) = tmpfs.rollback() {
                tmpfs.state = MountState::Retained;
                return Err(err.context(format!("template rollback failed: {rollback_err:#}")));
            }
            return Err(err);
        }

        tmpfs.state = MountState::Retained;
        drop(tmpfs);
        Ok(Box::new(t))
    }

    /// Read as JSON: parsing it into `VmConfig` would change `create_container_timeout`.
    pub(crate) fn load_vm_config(template_path: &Path) -> Result<serde_json::Value> {
        let path = template_path.join(TEMPLATE_VM_CONFIG_FILE);
        let content =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;

        serde_json::from_slice(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    pub(crate) fn save_vm_config(&self) -> Result<()> {
        let path = self.state_path.join(TEMPLATE_VM_CONFIG_FILE);
        let content = serde_json::to_vec(&vm_config_value(&self.config)?)?;

        std::fs::write(&path, content)
            .with_context(|| format!("failed to write {}", path.display()))
    }

    pub fn template_vm_exists(&self) -> bool {
        if !self.memory_path().exists() || !self.device_state_path().exists() {
            return false;
        }

        self.config_path().is_none_or(|path| path.exists())
    }

    fn memory_path(&self) -> PathBuf {
        self.state_path.join("memory")
    }

    fn device_state_path(&self) -> PathBuf {
        template_device_state_path(&self.config.hypervisor_name, &self.state_path)
    }

    fn config_path(&self) -> Option<PathBuf> {
        match self.config.hypervisor_name.as_str() {
            HYPERVISOR_NAME_CH => Some(self.state_path.join("config.json")),
            _ => None,
        }
    }

    /// Configures the VM configuration for template operations.
    fn prepare_vm_config(&self, boot_to_be_template: bool) -> VmConfig {
        let mut config = self.config.clone();
        config.hypervisor_config.vm_template.boot_to_be_template = boot_to_be_template;
        config.hypervisor_config.vm_template.boot_from_template = !boot_to_be_template;
        config.hypervisor_config.vm_template.memory_path =
            self.memory_path().to_string_lossy().to_string();
        config.hypervisor_config.vm_template.device_state_path =
            self.device_state_path().to_string_lossy().to_string();
        config
    }

    async fn save_to_template(
        &self,
        toml_config: TomlConfig,
        tmpfs: &mut TemplateMount<'_>,
    ) -> Result<()> {
        let config = self.prepare_vm_config(true);
        let (sender, _receiver) = channel(MESSAGE_BUFFER_SIZE);
        let vm = TemplateVm::prepare(config, toml_config, sender)
            .await
            .context("prepare template vm")?;

        // A cancelled start or an unconfirmed exit must leave the snapshot mount in place.
        tmpfs.state = MountState::Retained;
        let save_result: Result<()> = async {
            vm.start().await?;
            vm.disconnect().await.context("disconnect template vm")?;

            // Sleep a bit to let the agent grpc server clean up
            // See: src/runtime/virtcontainers/factory/template/template_linux.go#L139-L145
            // When we close connection to the agent, it needs sometime to cleanup
            // and restart listening on the communication( serial or vsock) port.
            // That time can be saved if we sleep a bit to wait for the agent to
            // come around and start listening again. The sleep is only done when
            // creating new vm templates and saves time for every new vm that are
            // created from template, so it worth the invest.
            sleep(TEMPLATE_WAIT_FOR_AGENT).await;

            vm.pause().await.context("pause template vm")?;
            vm.save().await.context("save template vm")
        }
        .await;

        let teardown_result = vm.teardown().await;
        if vm.vmm_exit_confirmed() {
            tmpfs.state = MountState::Mounted;
        }

        match (save_result, teardown_result) {
            (Ok(()), teardown) => teardown,
            (Err(err), Ok(())) => Err(err),
            (Err(err), Err(teardown_err)) => {
                Err(err.context(format!("template teardown also failed: {teardown_err:#}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_preserves_preexisting_files() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("existing");
        fs::write(&existing, b"keep me").unwrap();

        assert!(TemplateMount::prepare(dir.path(), 1).is_err());
        assert_eq!(fs::read(existing).unwrap(), b"keep me");
    }

    #[test]
    fn concurrent_factory_operation_gets_busy() {
        let dir = tempfile::tempdir().unwrap();
        let template_path = dir.path().join("template");
        let lock = lock_template_parent(&template_path).unwrap();
        let err = lock_template_parent(&template_path).unwrap_err();
        assert_eq!(err.downcast_ref::<Errno>(), Some(&Errno::EWOULDBLOCK));
        drop(lock);
        assert!(lock_template_parent(&template_path).is_ok());
    }

    #[test]
    fn retained_template_directory_survives_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("template");
        fs::create_dir(&path).unwrap();
        drop(TemplateMount {
            path: &path,
            created_dir: true,
            state: MountState::Retained,
        });
        assert!(path.is_dir());
    }
}
