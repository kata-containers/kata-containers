// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use hypervisor::HYPERVISOR_NAME_CH;
use kata_sys_util::mount::umount_all;
use kata_types::config::TomlConfig;
use nix::fcntl::{renameat, Flock, FlockArg, AT_FDCWD};
use nix::mount::{mount, MsFlags};
use serde::{Deserialize, Serialize};
use slog::warn;

use crate::factory::{
    template_device_state_path,
    vm::{TemplateVm, VmConfig},
};

/// Compatibility delay after disconnecting from the source VM's agent.
///
/// Dragonball now resets snapshotted vsock connections on restore, but that
/// alone does not repair every guest userspace state captured while the old
/// ttrpc connection is closing. Keep this delay until a deterministic
/// guest-side snapshot barrier replaces the heuristic.
///
/// The duration is deliberately unchanged from the value this code has always
/// used. Lengthening it is not a correctness improvement: template creation
/// with a five-second delay was observed to produce a bad artifact, so the
/// delay is not what makes the snapshot safe. Artifact validation below is.
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);
const TEMPLATE_VALIDATION_ATTEMPTS: usize = 3;
/// A healthy restored agent answers Check immediately. Bound this separately
/// from the general-purpose agent RPC timeout so one bad artifact does not
/// stall every factory retry for roughly a minute.
const TEMPLATE_VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);
const TEMPLATE_READY_FILE: &str = "READY";
const TEMPLATE_READY_TMP_FILE: &str = ".READY.tmp";
const TEMPLATE_MANIFEST_VERSION: u16 = 1;

/// Preallocated size (in MB) for saving the device state snapshot of the template VM.
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8;
const MIB: u64 = 1024 * 1024;

/// Duplicate a [`TomlConfig`], which does not implement `Clone`.
///
/// Each creation attempt needs one configuration for the source VM and one for
/// the validation VM, so the value cannot simply be moved. The round trip
/// carries only what `TomlConfig` serializes: a field marked `#[serde(skip)]`
/// would be silently reset to its default. Nothing in the template path relies
/// on such a field today; prefer deriving `Clone` upstream over widening this.
fn copy_toml_config(config: &TomlConfig) -> Result<TomlConfig> {
    let value = serde_json::to_value(config).context("serialize VM template configuration")?;
    serde_json::from_value(value).context("deserialize VM template configuration")
}

#[derive(Debug, Deserialize, Serialize)]
struct TemplateManifest {
    version: u16,
    hypervisor: String,
}

/// Verify that template creation and provisional restore validation completed.
pub(crate) fn validate_template_ready(template_path: &Path, hypervisor_name: &str) -> Result<()> {
    let ready_path = template_path.join(TEMPLATE_READY_FILE);
    let manifest: TemplateManifest = serde_json::from_slice(
        &std::fs::read(&ready_path)
            .with_context(|| format!("read VM template readiness manifest {ready_path:?}"))?,
    )
    .with_context(|| format!("parse VM template readiness manifest {ready_path:?}"))?;

    if manifest.version != TEMPLATE_MANIFEST_VERSION {
        return Err(anyhow!(
            "unsupported VM template manifest version {}, expected {}",
            manifest.version,
            TEMPLATE_MANIFEST_VERSION
        ));
    }
    if manifest.hypervisor != hypervisor_name {
        return Err(anyhow!(
            "VM template was created for hypervisor '{}', not '{}'",
            manifest.hypervisor,
            hypervisor_name
        ));
    }

    Ok(())
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

        t.validate_template_files()
            .context("no ready VM template found")?;

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
        let _creation_lock = t.acquire_creation_lock()?;

        if t.template_vm_exists() {
            return Err(anyhow!(
                "There is already a VM template in {:?}",
                t.state_path
            ));
        }
        if t.state_path.exists() {
            t.cleanup_template_files()
                .context("discard incomplete VM template")?;
        }

        for attempt in 1..=TEMPLATE_VALIDATION_ATTEMPTS {
            // Clone the non-Clone configuration before mounting the artifact
            // path so a serialization error cannot leave a tmpfs behind.
            let save_config = copy_toml_config(&toml_config)?;
            let validation_config = copy_toml_config(&toml_config)?;
            t.prepare_template_files()
                .context("prepare template files")?;
            if let Err(save_error) = t.save_to_template(save_config).await {
                let cleanup_result = t.cleanup_template_files();
                if let Err(cleanup_error) = cleanup_result {
                    return Err(anyhow!(
                        "create template files: {save_error:#}; cleanup failed: {cleanup_error:#}"
                    ));
                }
                return Err(save_error).context("create template files");
            }

            match t.validate_saved_template(validation_config).await {
                Ok(()) => {
                    if let Err(publication_error) = t.write_ready_manifest() {
                        let cleanup_result = t.cleanup_template_files();
                        if let Err(cleanup_error) = cleanup_result {
                            return Err(anyhow!(
                                "publish VM template: {publication_error:#}; cleanup failed: \
                                 {cleanup_error:#}"
                            ));
                        }
                        return Err(publication_error).context("publish VM template");
                    }
                    return Ok(Box::new(t));
                }
                Err(validation_error) => {
                    t.cleanup_template_files().with_context(|| {
                        format!(
                            "discard template after validation attempt {attempt} failed: \
                             {validation_error:#}"
                        )
                    })?;
                    if attempt == TEMPLATE_VALIDATION_ATTEMPTS {
                        return Err(validation_error).context(format!(
                            "validate saved VM template after {TEMPLATE_VALIDATION_ATTEMPTS} attempts"
                        ));
                    }
                    warn!(
                        sl!(),
                        "saved VM template failed validation on attempt {}, recreating: {}",
                        attempt,
                        validation_error
                    );
                }
            }
        }

        // Unreachable while TEMPLATE_VALIDATION_ATTEMPTS > 0: the loop either
        // returns a published template or the final attempt's error. Return an
        // error rather than panicking should that constant ever become zero.
        Err(anyhow!(
            "VM template validation made no attempts:              TEMPLATE_VALIDATION_ATTEMPTS is {TEMPLATE_VALIDATION_ATTEMPTS}"
        ))
    }

    pub fn template_vm_exists(&self) -> bool {
        self.validate_template_files().is_ok()
    }

    fn validate_template_files(&self) -> Result<()> {
        if !self.memory_path().exists() {
            return Err(anyhow!("VM template memory file is missing"));
        }
        if !self.device_state_path().exists() {
            return Err(anyhow!("VM template device state is missing"));
        }
        if let Some(path) = self.config_path() {
            if !path.exists() {
                return Err(anyhow!("VM template hypervisor configuration is missing"));
            }
        }

        validate_template_ready(&self.state_path, &self.config.hypervisor_name)
    }

    fn acquire_creation_lock(&self) -> Result<Flock<File>> {
        let parent = self
            .state_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create VM template parent directory {parent:?}"))?;

        let file_name = self
            .state_path
            .file_name()
            .ok_or_else(|| anyhow!("VM template path has no final component"))?;
        let mut lock_name = file_name.to_os_string();
        lock_name.push(".lock");
        let lock_path = parent.join(lock_name);
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("open VM template creation lock {lock_path:?}"))?;

        Flock::lock(lock_file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
            anyhow!(
                "VM template creation is already in progress for {:?}: {}",
                self.state_path,
                error
            )
        })
    }

    fn write_ready_manifest(&self) -> Result<()> {
        let manifest = TemplateManifest {
            version: TEMPLATE_MANIFEST_VERSION,
            hypervisor: self.config.hypervisor_name.clone(),
        };
        let contents = serde_json::to_vec_pretty(&manifest)
            .context("serialize VM template readiness manifest")?;
        let temporary_path = self.state_path.join(TEMPLATE_READY_TMP_FILE);
        let ready_path = self.state_path.join(TEMPLATE_READY_FILE);
        let mut file = File::create(&temporary_path)
            .with_context(|| format!("create VM template readiness manifest {temporary_path:?}"))?;
        file.write_all(&contents)
            .with_context(|| format!("write VM template readiness manifest {temporary_path:?}"))?;
        file.sync_all()
            .with_context(|| format!("sync VM template readiness manifest {temporary_path:?}"))?;
        renameat(AT_FDCWD, &temporary_path, AT_FDCWD, &ready_path).with_context(|| {
            format!("publish VM template readiness manifest {temporary_path:?} as {ready_path:?}")
        })?;

        Ok(())
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

    pub fn prepare_template_files(&self) -> Result<()> {
        // Create state directory
        std::fs::create_dir_all(&self.state_path)
            .context(format!("failed to create directory: {:?}", self.state_path))?;

        // Verify directory was created and is accessible
        if !self.state_path.exists() {
            return Err(anyhow!(
                "state path {:?} does not exist after creation",
                self.state_path
            ));
        }

        // Mount tmpfs to store template VM memory data in memory for:
        // - Accelerating VM cloning by avoiding disk I/O
        // - Enhancing security by keeping sensitive data in memory
        // - Supporting QEMU's shared memory clone model
        let opts = format!(
            "size={}M",
            self.config.hypervisor_config.memory_info.default_memory
                + TEMPLATE_DEVICE_STATE_SIZE_MB
        );
        mount(
            Some("tmpfs"),
            &self.state_path,
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some(opts.as_str()),
        )
        .context(format!("failed to mount tmpfs at {:?}", self.state_path))?;

        // Verify mount successfully by checking if directory is still accessible
        if !self.state_path.is_dir() {
            return Err(anyhow!(
                "state path {:?} is not a directory after mount",
                self.state_path
            ));
        }

        // Create memory file
        let memory_file = self.memory_path();
        let file = File::create(&memory_file)
            .context(format!("failed to create memory file: {memory_file:?}"))?;
        let memory_size_bytes = u64::from(self.config.hypervisor_config.memory_info.default_memory)
            .checked_mul(MIB)
            .ok_or_else(|| anyhow!("template memory size overflows u64"))?;
        file.set_len(memory_size_bytes)
            .context(format!("failed to size memory file: {memory_file:?}"))?;

        // Verify memory file was created successfully
        if !memory_file.exists() {
            return Err(anyhow!(
                "memory file {:?} does not exist after creation",
                memory_file
            ));
        }

        Ok(())
    }

    fn cleanup_template_files(&self) -> Result<()> {
        if !self.state_path.exists() {
            return Ok(());
        }

        if let Err(normal_error) = umount_all(&self.state_path, false) {
            warn!(
                sl!(),
                "normal unmount of failed VM template {:?} failed: {}",
                self.state_path,
                normal_error
            );
            umount_all(&self.state_path, true).context("lazy unmount failed VM template")?;
        }
        std::fs::remove_dir_all(&self.state_path).context("remove failed VM template directory")?;
        Ok(())
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

    pub async fn save_to_template(&self, toml_config: TomlConfig) -> Result<()> {
        let config = self.prepare_vm_config(true);
        let vm = TemplateVm::new_vm(config, toml_config)
            .await
            .context("new template vm")?;

        // Save the result so teardown still runs if any creation step fails.
        let result: Result<()> = async {
            vm.agent
                .check(agent::CheckRequest::new(""))
                .await
                .context("check template VM agent")?;

            vm.disconnect().await.context("disconnect template vm")?;

            // Use an async timer so template creation does not block a Tokio
            // worker thread. This remains a compatibility measure rather than
            // a correctness proof; the saved artifact is validated below.
            tokio::time::sleep(TEMPLATE_WAIT_FOR_AGENT).await;

            vm.pause().await.context("pause template vm")?;
            vm.save().await.context("save template vm")
        }
        .await;

        let teardown_result = vm.teardown().await;

        if let Err(teardown_error) = teardown_result {
            if result.is_ok() {
                return Err(teardown_error);
            }
            warn!(
                sl!(),
                "failed to tear down template VM after template creation failed: {}",
                teardown_error
            );
        }

        result
    }

    async fn validate_saved_template(&self, toml_config: TomlConfig) -> Result<()> {
        let config = self.prepare_vm_config(false);
        let vm = TemplateVm::new_vm(config, toml_config)
            .await
            .context("restore provisional template")?;

        let result: Result<()> = async {
            let check_result = tokio::time::timeout(
                TEMPLATE_VALIDATION_TIMEOUT,
                vm.agent.check(agent::CheckRequest::new("")),
            )
            .await
            .context("timed out checking restored template VM agent")?;
            check_result.context("check restored template VM agent")?;
            Ok(())
        }
        .await;
        let teardown_result = vm.teardown().await;

        match (result, teardown_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(validation_error), Ok(())) => Err(validation_error),
            (Ok(()), Err(teardown_error)) => {
                Err(teardown_error).context("tear down validated template VM")
            }
            (Err(validation_error), Err(teardown_error)) => Err(anyhow!(
                "validate restored template VM: {validation_error:#}; \
                 teardown failed: {teardown_error:#}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn test_template() -> (PathBuf, Template) {
        let root = std::env::temp_dir().join(format!("kata-template-test-{}", Uuid::new_v4()));
        let state_path = root.join("template");
        let config = VmConfig {
            hypervisor_name: "dragonball".to_string(),
            ..Default::default()
        };
        (root, Template::new(config, state_path))
    }

    #[test]
    fn test_ready_manifest_is_validated() {
        let (root, template) = test_template();
        std::fs::create_dir_all(&template.state_path).unwrap();

        template.write_ready_manifest().unwrap();
        validate_template_ready(&template.state_path, "dragonball").unwrap();
        assert!(validate_template_ready(&template.state_path, "qemu").is_err());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_creation_lock_is_exclusive() {
        let (root, template) = test_template();

        let lock = template.acquire_creation_lock().unwrap();
        assert!(template.acquire_creation_lock().is_err());
        drop(lock);
        template.acquire_creation_lock().unwrap();

        std::fs::remove_dir_all(root).unwrap();
    }
}
