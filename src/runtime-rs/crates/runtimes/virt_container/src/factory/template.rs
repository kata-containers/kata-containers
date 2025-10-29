// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::Debug;
use std::fs::File;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use kata_types::config::TomlConfig;
use nix::mount::{mount, MsFlags};

use crate::factory::vm::{TemplateVm, VmConfig};

/// Maximum time to wait for the Kata Agent to become ready when initializing a template VM.
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);

/// Preallocated size (in MB) for saving the device state snapshot of the template VM.
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8;

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

        if t.template_vm_exists() {
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

        Ok(Box::new(t))
    }

    pub fn template_vm_exists(&self) -> bool {
        let memory_path = self.state_path.join("memory");
        let state_path = self.state_path.join("state");

        memory_path.exists() && state_path.exists()
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
        let memory_file = self.state_path.join("memory");
        File::create(&memory_file)
            .context(format!("failed to create memory file: {:?}", memory_file))?;

        // Verify memory file was created successfully
        if !memory_file.exists() {
            return Err(anyhow!(
                "memory file {:?} does not exist after creation",
                memory_file
            ));
        }

        Ok(())
    }

    /// Configures the VM configuration for template operations.
    fn prepare_vm_config(&self, boot_to_be_template: bool) -> VmConfig {
        let mut config = self.config.clone();
        config.hypervisor_config.vm_template.boot_to_be_template = boot_to_be_template;
        config.hypervisor_config.vm_template.boot_from_template = !boot_to_be_template;
        config.hypervisor_config.vm_template.memory_path =
            self.state_path.join("memory").to_string_lossy().to_string();
        config.hypervisor_config.vm_template.device_state_path =
            self.state_path.join("state").to_string_lossy().to_string();
        config
    }

    pub async fn save_to_template(&self, toml_config: TomlConfig) -> Result<()> {
        let config = self.prepare_vm_config(true);
        let vm = TemplateVm::new_vm(config, toml_config)
            .await
            .context("new template vm")?;

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

        vm.save().await.context("save template vm")?;

        Ok(())
    }
}
