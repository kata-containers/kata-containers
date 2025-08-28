#![allow(unused_variables, unused_imports)]
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
use std::fmt::Debug;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use std::thread::sleep;
use std::time::Duration;

// use std::thread;
// use std::{thread, os::unix::fs::PermissionsExt};

use anyhow::{anyhow, Result};
// use async_trait::async_trait;
use async_trait::async_trait;
use nix::mount::{mount, MsFlags};
// use tokio::sync::Mutex;

use crate::factory::vm::{VMConfig, VM};
// use crate::runtime::protocols::cache::GrpcVMStatus;
// use crate::runtime::virtcontainers::factory::base::FactoryBase;
use kata_types::config::TomlConfig;
#[allow(dead_code)]
const TEMPLATE_WAIT_FOR_AGENT: Duration = Duration::from_secs(2);
const TEMPLATE_DEVICE_STATE_SIZE_MB: u32 = 8; // as in Go templateDeviceStateSize
use hypervisor::{qemu::Qemu, Hypervisor, HYPERVISOR_QEMU};
// use slog::{error};
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory"))
    };
}

pub trait FactoryBase: Debug {}

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

    async fn create_template_vm(&self, toml_config: TomlConfig) -> Result<()> {
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

        // // 在断开 gRPC 后 sleep 一会儿，给 agent 充分的时间清理资源并重新监听端口
        // // Sleep a bit to let the agent grpc server clean up
        // // When we close connection to the agent, it needs sometime to cleanup
        // // and restart listening on the communication( serial or vsock) port.
        // // That time can be saved if we sleep a bit to wait for the agent to
        // // come around and start listening again. The sleep is only done when
        // // creating new vm templates and saves time for every new vm that are
        // // created from template, so it worth the invest.
        // sleep(TEMPLATE_WAIT_FOR_AGENT);

        vm.pause().await?;
        info!(sl!(), "template::create_template_vm: pause()");

        vm.save().await?;
        info!(sl!(), "template::create_template_vm: save()");

        // vm.stop().await?;
        // info!(sl!(), "template::create_template_vm: stop()");

        Ok(())
    }

    // fn create_from_template_vm(&self, ctx: &tokio::runtime::Handle, c: &VMConfig) -> Result<VM> {
    //     let mut config = self.config.clone();
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
