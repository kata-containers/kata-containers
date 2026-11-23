// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use kata_types::config::TomlConfig;
use nix::mount::{mount, MsFlags};
use slog::warn;

use crate::factory::{
    vm::{snapshot_support, TemplateVm, VmConfig},
    vm_config_value,
};

/// Maximum time to wait for the Kata Agent to become ready when initializing a template VM.
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);

/// Headroom (in MB) on top of guest memory for the hypervisor's device-state artifacts.
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8;

/// Cloud Hypervisor already writes its own `config.json` here.
const TEMPLATE_VM_CONFIG_FILE: &str = "vm_config.json";

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

    /// Return the existing template at `template_path`, or an error if the
    /// hypervisor does not recognise a complete snapshot there.
    pub async fn fetch(
        config: VmConfig,
        toml_config: &TomlConfig,
        template_path: PathBuf,
    ) -> Result<Box<Template>> {
        let t = Template::new(config, template_path);

        t.validate_template(toml_config)
            .await
            .context("no template vm found")?;

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

        if t.validate_template(&toml_config).await.is_ok() {
            return Err(anyhow!(
                "There is already a VM template in {:?}",
                t.state_path
            ));
        }

        t.prepare_template_files()
            .context("prepare template files")?;
        t.save_to_template(toml_config)
            .await
            .context("create template files")?;
        t.save_vm_config().context("save template vm config")?;

        Ok(Box::new(t))
    }

    /// Ask the hypervisor whether `state_path` holds a complete template.
    pub async fn validate_template(&self, toml_config: &TomlConfig) -> Result<()> {
        let hypervisor = TemplateVm::new_hypervisor(&self.config, toml_config)
            .await
            .context("new hypervisor")?;
        snapshot_support(hypervisor.as_ref(), &self.config.hypervisor_name)?
            .validate_snapshot(&self.state_path)
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

        Ok(())
    }

    pub async fn save_to_template(&self, toml_config: TomlConfig) -> Result<()> {
        let vm = TemplateVm::new_vm(self.config.clone(), toml_config, &self.state_path)
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
            vm.save(&self.state_path).await.context("save template vm")
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
}
