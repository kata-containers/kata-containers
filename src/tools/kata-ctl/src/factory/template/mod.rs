#![allow(unused_variables, unused_imports)]
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
use std::fs::{File};
use std::path::PathBuf;
use std::fmt::Debug;
use std::sync::Arc;
// use std::time::Duration;
// use std::thread;
// use std::{thread, os::unix::fs::PermissionsExt};

use anyhow::{anyhow, Result};
// use async_trait::async_trait;
use async_trait::async_trait; 
use nix::mount::{mount, MsFlags};
// use tokio::sync::Mutex;

use crate::factory::vm::{VMConfig,VM};
// use crate::runtime::protocols::cache::GrpcVMStatus;
// use crate::runtime::virtcontainers::factory::base::FactoryBase;


// const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8; // as in Go templateDeviceStateSize
#[allow(unused_imports)]
use hypervisor::{qemu::Qemu, HYPERVISOR_QEMU, Hypervisor};


// use slog::{error};
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory"))
    };
}

pub trait FactoryBase : Debug {

}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Template {
    state_path: PathBuf,
    config: VMConfig,
}

impl Template { 
    pub fn fetch(config: VMConfig, template_path: PathBuf) -> Result<Box<dyn FactoryBase>> {
        let t = Template {
            state_path: template_path,
            config,
        };

        t.check_template_vm()?;
        Ok(Box::new(t))
    }

    pub async fn new(config: VMConfig, toml_config：TomlConfig, template_path: PathBuf) -> Result<Box<dyn FactoryBase>> {
        let t = Template {
            state_path: template_path,
            config,
        };

        match t.check_template_vm(toml_config) {
            Ok(_) => {
                return Err(anyhow!("There is already a VM template in {:?}", t.state_path));
            }
            Err(e) => {
                info!(sl!(), "check_template_vm failed as expected: {}", e);
            }
        }


        t.prepare_template_files()?;

        if let Err(e) = t.create_template_vm().await {
            // t.close()?;
            return Err(e);
        }

        Ok(Box::new(t))
    }

    // pub fn config(&self) -> &VMConfig {
    //     &self.config
    // }

    fn check_template_vm(&self) -> Result<()> {
        let memory_path = self.state_path.join("memory");
        let state_path = self.state_path.join("state");
        if !memory_path.exists() || !state_path.exists() {
            return Err(anyhow!("template VM memory or state file missing"));
        }
        Ok(())
    }

    // fn close(&self) -> Result<()> {
    //     if let Err(e) = umount2(&self.state_path, MntFlags::MNT_DETACH) {
    //         eprintln!("failed to unmount {}: {}", self.state_path.display(), e);
    //     }
    //     if let Err(e) = fs::remove_dir_all(&self.state_path) {
    //         eprintln!("failed to remove {}: {}", self.state_path.display(), e);
    //     }
    //     Ok(())
    // }

    fn prepare_template_files(&self) -> Result<()> {
        std::fs::create_dir_all(&self.state_path)?;
        let opts = format!("size={}M", self.config.hypervisor_config.memory_info.default_memory + TEMPLATE_DEVICE_STATE_SIZE_MB);
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

    async fn create_template_vm(&self, toml_config：TomlConfig) -> Result<()> {
        let mut config = self.config.clone();
        config.hypervisor_config.boot_to_be_template = true;
        config.hypervisor_config.boot_from_template = false;
        config.hypervisor_config.memory_path = self.state_path.join("memory").to_string_lossy().to_string();
        config.hypervisor_config.device_state_path = self.state_path.join("state").to_string_lossy().to_string();
        // config.new_vm();
        let vm = VM::new_vm(config, toml_config).await?;
        // let hypervisor: Arc<dyn Hypervisor> = match config.hypervisor_name.as_str() {
        //     HYPERVISOR_QEMU => {
        //         let h = Qemu::new();
        //         h.set_hypervisor_config(config.hypervisor_config.clone()).await;
        //         Arc::new(h)
        //     }
        //     _ => return Err(anyhow!("Unsupported hypervisor {}", config.hypervisor_name)),
        // };

        // info!(sl!(),"Created hypervisor: {:?}", hypervisor);
        
        // hypervisor.stop_vm().await?;
        // hypervisor.disconnect().await;

        // thread::sleep(TEMPLATE_WAIT_FOR_AGENT);

        // hypervisor.pause_vm().await?;
        // hypervisor.save_vm().await?;

        Ok(())
    }

    // fn create_from_template_vm(&self, ctx: &tokio::runtime::Handle, c: &VMConfig) -> Result<VM> {
    //     let mut config = self.config.clone();
    //     config.hypervisor_config.boot_to_be_template = false;
    //     config.hypervisor_config.boot_from_template = true;
    //     config.hypervisor_config.memory_path = Some(self.state_path.join("memory"));
    //     config.hypervisor_config.devices_state_path = Some(self.state_path.join("state"));
    //     config.hypervisor_config.shared_path = c.hypervisor_config.shared_path.clone();
    //     config.hypervisor_config.vm_store_path = c.hypervisor_config.vm_store_path.clone();
    //     config.hypervisor_config.run_store_path = c.hypervisor_config.run_store_path.clone();

    //     VM::new(ctx, &config)
    // }
}

#[async_trait]
impl FactoryBase for Template {
    // fn config(&self) -> &VMConfig {
    //     &self.config
    // }

    // fn close_factory(&self, _ctx: &tokio::runtime::Handle) -> Result<()> {
    //     self.close()
    // }

    // fn get_base_vm(&self, ctx: &tokio::runtime::Handle, config: &VMConfig) -> Result<VM> {
    //     self.create_from_template_vm(ctx, config)
    // }

    // fn get_vm_status(&self) -> Vec<GrpcVMStatus> {
    //     panic!("package template does not support GetVMStatus")
    // }
}