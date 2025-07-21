// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(unused_variables, unused_imports)]
use std::fmt::Debug;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use std::thread::sleep;
use std::time::Duration;

use anyhow::Context;
use anyhow::{anyhow, Result};

use async_trait::async_trait;
use nix::mount::{mount, MsFlags};

use crate::factory::vm::{VMConfig, VM};
use kata_types::config::TomlConfig;

#[allow(dead_code)]
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8; // as in Go templateDeviceStateSize
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory"))
    };
}

pub trait FactoryBase: Debug {}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Template {
    pub state_path: PathBuf,
    pub config: VMConfig,
}

impl Template {
    pub fn fetch(config: VMConfig, template_path: PathBuf) -> Result<Box<dyn FactoryBase>> {
        let t = Template {
            state_path: template_path,
            config,
        };

        // Call check_template_vm to validate the template's files
        if let Err(e) = t.check_template_vm() {
            // If check_template_vm returns an error, log the error and return a detailed error message
            return Err(anyhow!(e));
        }

        // If there is no error, return a Boxed instance of the Template
        Ok(Box::new(t))
    }

    pub async fn new(
        config: VMConfig,
        toml_config: TomlConfig,
        template_path: PathBuf,
    ) -> Result<Box<dyn FactoryBase>> {
        let t = Template {
            state_path: template_path,
            config,
        };

        match t.check_template_vm() {
            Ok(_) => {
                return Err(anyhow!(
                    "There is already a VM template in {:?}",
                    t.state_path
                ));
            }
            Err(e) => {
                info!(sl!(), "check_template_vm failed as expected: {}", e);
            }
        }

        t.prepare_template_files()?;

        if let Err(e) = t.create_template_vm(toml_config).await {
            // t.close()?;
            return Err(e);
        }

        Ok(Box::new(t))
    }

    pub fn check_template_vm(&self) -> Result<()> {
        let memory_path = self.state_path.join("memory");
        let state_path = self.state_path.join("state");

        if !memory_path.exists() || !state_path.exists() {
            info!(sl!(), "Template VM memory or state file missing");
            return Err(anyhow!("template VM memory or state file missing"));
        }

        Ok(())
    }

    pub fn prepare_template_files(&self) -> Result<()> {
        std::fs::create_dir_all(&self.state_path)?;
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
        )?;

        let memory_file = self.state_path.join("memory");
        File::create(memory_file)?;

        Ok(())
    }

    pub async fn create_template_vm(&self, toml_config: TomlConfig) -> Result<()> {
        info!(sl!(), "template::create_template_vm: start(): start");
        let mut config = self.config.clone();
        config.hypervisor_config.boot_to_be_template = true;
        config.hypervisor_config.boot_from_template = false;
        config.hypervisor_config.memory_path =
            self.state_path.join("memory").to_string_lossy().to_string();
        config.hypervisor_config.device_state_path =
            self.state_path.join("state").to_string_lossy().to_string();
        // config.new_vm();
        let vm = VM::new_vm(config, toml_config).await?;
        info!(
            sl!(),
            "template::create_template_vm: new_vm() VM id={}, cpu={}, memory={}",
            vm.id,
            vm.cpu,
            vm.memory
        );

        vm.disconnect().await?;
        info!(sl!(), "template::create_template_vm: disconnect()");

        // Sleep a bit to let the agent grpc server clean up
        // When we close connection to the agent, it needs sometime to cleanup
        // and restart listening on the communication( serial or vsock) port.
        // That time can be saved if we sleep a bit to wait for the agent to
        // come around and start listening again. The sleep is only done when
        // creating new vm templates and saves time for every new vm that are
        // created from template, so it worth the invest.
        sleep(TEMPLATE_WAIT_FOR_AGENT);

        vm.pause().await?;
        info!(sl!(), "template::create_template_vm: pause()");

        vm.save().await?;
        info!(sl!(), "template::create_template_vm: save()");

        // vm.stop().await?;
        // info!(sl!(), "template::create_template_vm: stop()");

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn create_from_template_vm(&self, new_config: &mut VMConfig) -> Result<VM> {
        info!(sl!(), "template::create_from_template_vm: start(): start");

        let mut config = self.config.clone();
        config.hypervisor_config.boot_to_be_template = false;
        config.hypervisor_config.boot_from_template = true;
        config.hypervisor_config.memory_path =
            self.state_path.join("memory").to_string_lossy().to_string();
        config.hypervisor_config.device_state_path =
            self.state_path.join("state").to_string_lossy().to_string();
        config.hypervisor_config.shared_path = new_config.hypervisor_config.shared_path.clone();
        config.hypervisor_config.vm_store_path = new_config.hypervisor_config.vm_store_path.clone();
        config.hypervisor_config.run_store_path =
            new_config.hypervisor_config.run_store_path.clone();

        let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

        let vm = VM::new_vm(config, toml_config).await?;
        info!(
            sl!(),
            "template::get_base_vm():  vm: new_vm() VM id={}, cpu={}, memory={}",
            vm.id,
            vm.cpu,
            vm.memory
        );
        Ok(vm)
    }

    pub async fn get_base_vm(&self, config: &mut VMConfig) -> Result<VM> {
        info!(sl!(), "template::get_base_vm(): start");
        let vm = self.create_from_template_vm(config).await?;
        info!(
            sl!(),
            "template::get_base_vm():  vm: new_vm() VM id={}, cpu={}, memory={}",
            vm.id,
            vm.cpu,
            vm.memory
        );
        Ok(vm)
    }
}

#[async_trait]
impl FactoryBase for Template {

}
