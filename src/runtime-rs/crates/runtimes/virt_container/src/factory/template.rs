// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use hypervisor::HYPERVISOR_NAME_CH;
use kata_types::config::TomlConfig;
use nix::mount::{mount, MsFlags};
use ring::digest::{Context as Sha256Context, SHA256};
use serde::{Deserialize, Serialize};
use slog::warn;

use crate::factory::{
    template_device_state_path,
    vm::{TemplateVm, VmConfig},
    vm_config_value,
};

/// Maximum time to wait for the Kata Agent to become ready when initializing a template VM.
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);

/// Preallocated size (in MB) for saving the device state snapshot of the template VM.
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8;
const MIB: u64 = 1024 * 1024;
const TEMPLATE_IDENTITY_FILE: &str = "template.json";
const TEMPLATE_IDENTITY_FORMAT_VERSION: u32 = 2;
/// Cloud Hypervisor already writes its own `config.json` in the template directory.
const TEMPLATE_VM_CONFIG_FILE: &str = "vm_config.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemplateIdentity {
    pub(crate) format_version: u32,
    pub(crate) hypervisor: String,
    pub(crate) memory_size: u64,
    pub(crate) memory_sha256: String,
    pub(crate) device_state_size: u64,
    pub(crate) device_state_sha256: String,
    pub(crate) config_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) native_config_sha256: Option<String>,
}

impl TemplateIdentity {
    /// Match the template configuration without reading deployed boot assets.
    pub(crate) fn verify_vm_config(&self, config: &VmConfig) -> Result<()> {
        if Template::config_sha256_for(config)? != self.config_sha256 {
            return Err(anyhow!("VM template configuration does not match"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Template {
    pub state_path: PathBuf,
    pub config: VmConfig,
    identity: Option<TemplateIdentity>,
}

impl Template {
    /// Creates a new Template instance with the given configuration and path.
    pub fn new(config: VmConfig, template_path: PathBuf) -> Self {
        Template {
            state_path: template_path,
            config,
            identity: None,
        }
    }

    pub fn fetch(config: VmConfig, template_path: PathBuf) -> Result<Box<Template>> {
        Self::fetch_inner(config, template_path, true)
    }

    fn fetch_inner(
        config: VmConfig,
        template_path: PathBuf,
        verify_assets: bool,
    ) -> Result<Box<Template>> {
        let mut t = Template::new(config, template_path);

        if !t.template_vm_exists()? {
            return Err(anyhow!(
                "VM template metadata is incomplete; recreate the template"
            ));
        }

        t.identity = Some(t.load_identity(verify_assets)?);

        Ok(Box::new(t))
    }

    /// Creates and saves a new template VM to disk.
    /// This will prepare template files, create a VM, and save its state.
    pub async fn create(
        config: VmConfig,
        toml_config: TomlConfig,
        template_path: PathBuf,
    ) -> Result<Box<Template>> {
        let mut t = Template::new(config, template_path);

        if t.template_vm_exists()? {
            return Err(anyhow!(
                "There is already a VM template in {:?}",
                t.state_path
            ));
        }

        if t.state_path.exists()
            && t.state_path
                .read_dir()
                .with_context(|| format!("read template directory {}", t.state_path.display()))?
                .next()
                .is_some()
        {
            return Err(anyhow!(
                "incomplete or incompatible VM template exists in {}; destroy it before recreating",
                t.state_path.display()
            ));
        }

        t.prepare_template_files()
            .context("prepare template files")?;
        t.save_to_template(toml_config)
            .await
            .context("create template files")?;
        t.save_vm_config().context("save template vm config")?;
        t.identity = Some(t.publish_identity().context("publish template identity")?);

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

    pub fn template_vm_exists(&self) -> Result<bool> {
        if !self.memory_path().exists()
            || !self.device_state_path()?.exists()
            || !self.identity_path().exists()
            || !self.state_path.join(TEMPLATE_VM_CONFIG_FILE).exists()
        {
            return Ok(false);
        }

        Ok(self.config_path().is_none_or(|path| path.exists()))
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

        let memory_file = self.memory_path();
        let file = File::create(&memory_file)
            .context(format!("failed to create memory file: {memory_file:?}"))?;
        let memory_size_bytes = u64::from(self.config.hypervisor_config.memory_info.default_memory)
            .checked_mul(MIB)
            .ok_or_else(|| anyhow!("template memory size overflows u64"))?;
        file.set_len(memory_size_bytes)
            .context(format!("failed to size memory file: {memory_file:?}"))?;

        Ok(())
    }

    fn template_vm_config(&self) -> Result<VmConfig> {
        let mut config = self.config.clone();

        config.hypervisor_config.vm_template.boot_to_be_template = true;
        config.hypervisor_config.vm_template.boot_from_template = false;
        config.hypervisor_config.vm_template.memory_path =
            self.memory_path().to_string_lossy().to_string();
        config.hypervisor_config.vm_template.device_state_path =
            self.device_state_path()?.to_string_lossy().to_string();

        Ok(config)
    }

    pub async fn save_to_template(&self, toml_config: TomlConfig) -> Result<()> {
        let config = self.template_vm_config()?;
        let vm = TemplateVm::new_vm(config, toml_config)
            .await
            .context("new template vm")?;

        // Save the result so teardown still runs if any creation step fails.
        let result: Result<()> = async {
            vm.disconnect().await.context("disconnect template vm")?;

            // Sleep a bit to let the agent grpc server clean up
            // See: src/runtime/virtcontainers/factory/template/template_linux.go#L139-L145
            // When we close connection to the agent, it needs sometime to cleanup
            // and restart listening on the communication( serial or vsock) port.
            // That time can be saved if we sleep a bit to wait for the agent to
            // come around and start listening again. The sleep is only done when
            // creating new vm templates and saves time for every new vm that are
            // created from template, so it worth the invest.
            sleep(TEMPLATE_WAIT_FOR_AGENT);

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

    fn memory_path(&self) -> PathBuf {
        self.state_path.join("memory")
    }

    fn device_state_path(&self) -> Result<PathBuf> {
        Ok(template_device_state_path(
            self.config.hypervisor_name.as_str(),
            &self.state_path,
        ))
    }

    fn identity_path(&self) -> PathBuf {
        self.state_path.join(TEMPLATE_IDENTITY_FILE)
    }

    fn file_integrity(path: &std::path::Path) -> Result<(u64, String)> {
        let metadata = std::fs::symlink_metadata(path)
            .with_context(|| format!("inspect template artifact {}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(anyhow!(
                "template artifact {} is not a regular file",
                path.display()
            ));
        }

        let file =
            File::open(path).context(format!("open template artifact {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256Context::new(&SHA256);
        let mut buffer = vec![0_u8; 1024 * 1024];

        loop {
            let count = reader
                .read(&mut buffer)
                .with_context(|| format!("read template artifact {}", path.display()))?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        let sha256 = hasher
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        Ok((metadata.len(), sha256))
    }

    fn config_sha256(&self) -> Result<String> {
        Self::config_sha256_for(&self.config)
    }

    fn config_sha256_for(config: &VmConfig) -> Result<String> {
        let mut config = config.clone();
        // Deployed asset contents are validated by the asset manager.
        let hv = &mut config.hypervisor_config;
        hv.vm_template.memory_path.clear();
        hv.vm_template.device_state_path.clear();
        hv.factory.template_path.clear();

        // Host annotation authorization policy does not describe saved VM state.
        hv.valid_hypervisor_paths.clear();
        hv.valid_ctlpaths.clear();
        hv.valid_jailer_paths.clear();

        let bytes = serde_json::to_vec(&config).context("encode VM template configuration")?;
        let mut hasher = Sha256Context::new(&SHA256);
        hasher.update(&bytes);

        Ok(hasher
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    fn calculate_identity(&self) -> Result<TemplateIdentity> {
        let (memory_size, memory_sha256) = Self::file_integrity(&self.memory_path())?;
        let (device_state_size, device_state_sha256) =
            Self::file_integrity(&self.device_state_path()?)?;
        let native_config_sha256 = self
            .config_path()
            .map(|path| Self::file_integrity(&path).map(|(_, sha256)| sha256))
            .transpose()?;

        Ok(TemplateIdentity {
            format_version: TEMPLATE_IDENTITY_FORMAT_VERSION,
            hypervisor: self.config.hypervisor_name.clone(),
            memory_size,
            memory_sha256,
            device_state_size,
            device_state_sha256,
            config_sha256: self.config_sha256()?,
            native_config_sha256,
        })
    }

    fn publish_identity(&self) -> Result<TemplateIdentity> {
        let identity = self.calculate_identity()?;

        let identity_path = self.identity_path();
        let partial_path = self.state_path.join("template.json.partial");

        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&partial_path)
                .with_context(|| format!("create template identity {}", partial_path.display()))?;

            file.write_all(&serde_json::to_vec_pretty(&identity)?)
                .context("write template identity")?;

            file.sync_all().context("sync template identity")?;

            std::fs::rename(&partial_path, &identity_path).context("publish template identity")?;

            File::open(&self.state_path)?
                .sync_all()
                .context("sync template directory")?;

            Ok(())
        })();

        if result.is_err() {
            let _ = std::fs::remove_file(&partial_path);
        }
        result?;

        Ok(identity)
    }

    #[cfg(test)]
    fn load_and_verify_identity(&self) -> Result<TemplateIdentity> {
        self.load_identity(true)
    }

    fn load_identity(&self, verify_assets: bool) -> Result<TemplateIdentity> {
        let identity_path = self.identity_path();
        let metadata = std::fs::symlink_metadata(&identity_path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(anyhow!("VM template identity is not a regular file"));
        }

        let identity: TemplateIdentity = serde_json::from_slice(&std::fs::read(&identity_path)?)
            .context("decode template identity")?;

        if identity.format_version != TEMPLATE_IDENTITY_FORMAT_VERSION {
            return Err(anyhow!(
                "unsupported VM template identity format {}; recreate the template",
                identity.format_version
            ));
        }

        let regular_file_size = |path: &std::path::Path| -> Result<u64> {
            let metadata = std::fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(anyhow!(
                    "template artifact {} is not a regular file",
                    path.display()
                ));
            }

            Ok(metadata.len())
        };

        // Rehash the small device-state artifact on every fetch.
        let memory_size = regular_file_size(&self.memory_path())?;
        let (device_state_size, device_state_sha256) =
            Self::file_integrity(&self.device_state_path()?)?;
        let config_matches = !verify_assets || self.config_sha256()? == identity.config_sha256;
        let valid_digest = |digest: &str| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        };

        match (self.config_path(), identity.native_config_sha256.as_ref()) {
            (Some(path), Some(expected))
                if valid_digest(expected) && Self::file_integrity(&path)?.1 == *expected => {}
            (None, None) => {}
            _ => return Err(anyhow!("VM template native configuration does not match")),
        }

        if memory_size != identity.memory_size
            || device_state_size != identity.device_state_size
            || device_state_sha256 != identity.device_state_sha256
            || !config_matches
            || identity.hypervisor != self.config.hypervisor_name
            || !valid_digest(&identity.memory_sha256)
            || !valid_digest(&identity.device_state_sha256)
            || !valid_digest(&identity.config_sha256)
        {
            return Err(anyhow!("VM template identity does not match its artifacts"));
        }

        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_template() -> Template {
        let path = std::env::temp_dir().join(format!(
            "kata-template-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("memory"), b"template memory").unwrap();
        std::fs::write(path.join("state"), b"template device state").unwrap();
        let mut config = VmConfig {
            hypervisor_name: hypervisor::HYPERVISOR_QEMU.to_string(),
            ..Default::default()
        };
        config.hypervisor_config.memory_info.default_memory = 2048;
        let template = Template::new(config, path);
        template.save_vm_config().unwrap();
        template
    }

    #[test]
    fn template_identity_round_trip_and_artifact_mismatch() {
        let mut template = test_template();
        let identity = template.publish_identity().unwrap();
        template.identity = Some(identity.clone());
        assert_eq!(template.identity.as_ref().unwrap(), &identity);

        let fetched =
            Template::fetch(template.config.clone(), template.state_path.clone()).unwrap();
        assert_eq!(fetched.identity.as_ref().unwrap(), &identity);

        std::fs::write(template.memory_path(), b"modified template memory").unwrap();
        let error = Template::fetch(template.config.clone(), template.state_path.clone())
            .expect_err("modified template memory must fail identity validation");
        assert!(error
            .to_string()
            .contains("VM template identity does not match its artifacts"));
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn template_identity_verifies_configuration_without_boot_assets() {
        let mut template = test_template();
        template.config.hypervisor_config.boot_info.initrd = template
            .state_path
            .join("missing-initrd")
            .display()
            .to_string();
        template.save_vm_config().unwrap();
        let identity = template.publish_identity().unwrap();
        identity.verify_vm_config(&template.config).unwrap();
        Template::fetch(template.config.clone(), template.state_path.clone()).unwrap();

        let mut incompatible = template.config.clone();
        incompatible.hypervisor_config.memory_info.default_memory += 128;
        assert!(identity.verify_vm_config(&incompatible).is_err());
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn clh_template_binds_native_config() {
        let mut template = test_template();
        template.config.hypervisor_name = HYPERVISOR_NAME_CH.to_string();
        std::fs::rename(
            template.state_path.join("state"),
            template.state_path.join("state.json"),
        )
        .unwrap();
        let native_config = template.state_path.join("config.json");
        std::fs::write(&native_config, b"native config A").unwrap();
        template.save_vm_config().unwrap();
        let mut identity = template.publish_identity().unwrap();
        assert_eq!(identity.format_version, TEMPLATE_IDENTITY_FORMAT_VERSION);
        assert!(identity.native_config_sha256.is_some());
        Template::fetch(template.config.clone(), template.state_path.clone()).unwrap();

        // Per-sandbox edits must not change the original template identity.
        let private_config = template.state_path.join("private-config.json");
        std::fs::copy(&native_config, &private_config).unwrap();
        std::fs::write(&private_config, b"private socket").unwrap();
        Template::fetch(template.config.clone(), template.state_path.clone()).unwrap();
        std::fs::write(&native_config, b"native config B").unwrap();
        assert!(Template::fetch(template.config.clone(), template.state_path.clone()).is_err());
        std::fs::write(&native_config, b"native config A").unwrap();

        identity.native_config_sha256 = None;
        std::fs::write(
            template.identity_path(),
            serde_json::to_vec(&identity).unwrap(),
        )
        .unwrap();
        assert!(
            Template::fetch(template.config.clone(), template.state_path.clone())
                .unwrap_err()
                .to_string()
                .contains("native configuration does not match")
        );
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn template_identity_rejects_config_mismatch() {
        let mut template = test_template();
        template.publish_identity().unwrap();
        template.config.hypervisor_config.memory_info.default_memory += 1;

        let error = Template::fetch(template.config.clone(), template.state_path.clone())
            .expect_err("modified template configuration must fail identity validation");
        assert!(error
            .to_string()
            .contains("VM template identity does not match its artifacts"));
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn template_identity_binds_paths_but_not_boot_asset_contents() {
        let mut template = test_template();
        let asset = template.state_path.join("initrd");
        std::fs::write(&asset, b"agent version A").unwrap();
        template.config.hypervisor_config.boot_info.initrd = asset.display().to_string();
        template.publish_identity().unwrap();
        let original = template.config_sha256().unwrap();
        std::fs::write(&asset, b"agent version B").unwrap();
        assert_eq!(original, template.config_sha256().unwrap());
        template.load_and_verify_identity().unwrap();

        template.config.hypervisor_config.factory.template_path = "/other/template".into();
        assert_eq!(original, template.config_sha256().unwrap());
        template
            .config
            .hypervisor_config
            .boot_info
            .initrd
            .push_str(".relocated");
        assert_ne!(original, template.config_sha256().unwrap());
        assert!(template.load_and_verify_identity().is_err());
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn template_identity_ignores_host_path_authorization_policy() {
        let mut template = test_template();
        let identity = template.publish_identity().unwrap();
        let hv = &mut template.config.hypervisor_config;
        hv.valid_hypervisor_paths = vec!["/other/qemu/*".into()];
        hv.valid_ctlpaths = vec!["/other/control/*".into()];
        hv.valid_jailer_paths = vec!["/other/jailer/*".into()];
        assert_eq!(identity.config_sha256, template.config_sha256().unwrap());
        template.load_and_verify_identity().unwrap();
        template.config.hypervisor_config.cpu_info.default_maxvcpus += 1;
        assert!(template.load_and_verify_identity().is_err());
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn template_identity_rejects_same_size_device_state_mismatch() {
        let template = test_template();
        template.publish_identity().unwrap();
        let state_path = template.device_state_path().unwrap();
        let mut state = std::fs::read(&state_path).unwrap();
        state[0] ^= 0xff;
        std::fs::write(&state_path, state).unwrap();

        let error = Template::fetch(template.config.clone(), template.state_path.clone())
            .expect_err("same-size device-state corruption must fail identity validation");
        assert!(error
            .to_string()
            .contains("VM template identity does not match its artifacts"));
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }

    #[test]
    fn template_identity_rejects_unknown_format() {
        let template = test_template();
        let mut identity = template.publish_identity().unwrap();
        for version in [1, TEMPLATE_IDENTITY_FORMAT_VERSION + 1] {
            identity.format_version = version;
            std::fs::write(
                template.identity_path(),
                serde_json::to_vec(&identity).unwrap(),
            )
            .unwrap();
            let error = Template::fetch(template.config.clone(), template.state_path.clone())
                .expect_err("old or unknown identity format must fail");
            assert!(error
                .to_string()
                .contains("unsupported VM template identity format"));
        }
        std::fs::remove_dir_all(&template.state_path).unwrap();
    }
}
