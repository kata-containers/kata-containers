//Copyright (c) 2019-2022 Alibaba Cloud
//Copyright (c) 2023 Nubificus Ltd
//
//SPDX-License-Identifier: Apache-2.0

use crate::firecracker::{inner_hypervisor::FC_API_SOCKET_NAME, sl};
use crate::HypervisorState;
use crate::MemoryConfig;
use crate::HYPERVISOR_FIRECRACKER;
use crate::{device::DeviceType, VmmState};
use anyhow::{Context, Result};
use async_trait::async_trait;
use hyper::Client;
use hyperlocal::{UnixClientExt, UnixConnector};
use kata_types::{
    capabilities::{Capabilities, CapabilityBits},
    config::hypervisor::Hypervisor as HypervisorConfig,
};
use nix::sched::{setns, CloneFlags};
use persist::sandbox_persist::Persist;
use std::os::unix::io::AsRawFd;
use tokio::process::Command;

unsafe impl Send for FcInner {}
unsafe impl Sync for FcInner {}

#[derive(Debug)]
pub struct FcInner {
    pub(crate) id: String,
    pub(crate) asock_path: String,
    pub(crate) state: VmmState,
    pub(crate) config: HypervisorConfig,
    pub(crate) pid: Option<u32>,
    pub(crate) vm_path: String,
    pub(crate) netns: Option<String>,
    pub(crate) client: Client<UnixConnector>,
    pub(crate) jailer_root: String,
    pub(crate) jailed: bool,
    pub(crate) run_dir: String,
    pub(crate) pending_devices: Vec<DeviceType>,
    pub(crate) capabilities: Capabilities,
}

impl FcInner {
    pub fn new() -> FcInner {
        let mut capabilities = Capabilities::new();
        capabilities.set(CapabilityBits::BlockDeviceSupport);
        FcInner {
            id: String::default(),
            asock_path: String::default(),
            state: VmmState::NotReady,
            config: Default::default(),
            pid: None,
            netns: None,
            vm_path: String::default(),
            client: Client::unix(),
            jailer_root: String::default(),
            jailed: false,
            run_dir: String::default(),
            pending_devices: vec![],
            capabilities,
        }
    }

    pub(crate) async fn prepare_vmm(&mut self, netns: Option<String>) -> Result<()> {
        let mut cmd: Command;
        self.netns = netns.clone();
        match self.jailed {
            true => {
                debug!(sl(), "Running Jailed");
                cmd = Command::new(&self.config.jailer_path);
                let api_socket = ["/run/", FC_API_SOCKET_NAME].join("/");
                let args = [
                    "--id",
                    &self.id,
                    "--gid",
                    "0",
                    "--uid",
                    "0",
                    "--exec-file",
                    &self.config.path,
                    "--chroot-base-dir",
                    &self.jailer_root,
                    "--",
                    "--api-sock",
                    &api_socket,
                ];
                cmd.args(args);
            }
            false => {
                debug!(sl(), "Running non-Jailed");
                cmd = Command::new(&self.config.path);
                cmd.args(["--api-sock", &self.asock_path]);
            }
        }
        debug!(sl(), "Exec: {:?}", cmd);

        // Make sure we're in the correct Network Namespace
        unsafe {
            let _pre = cmd.pre_exec(move || {
                if let Some(netns_path) = &netns {
                    debug!(sl(), "set netns for vmm master {:?}", &netns_path);
                    let netns_fd = std::fs::File::open(netns_path);
                    let _ = setns(netns_fd?.as_raw_fd(), CloneFlags::CLONE_NEWNET)
                        .context("set netns failed");
                }
                Ok(())
            });
        }

        let mut child = cmd.spawn()?;

        match child.id() {
            Some(id) => {
                let cur_tid = nix::unistd::gettid().as_raw() as u32;
                info!(
                    sl(),
                    "VMM spawned successfully: PID: {:?}, current TID: {:?}", id, cur_tid
                );
                self.pid = Some(id);
            }
            None => {
                let exit_status = child.wait().await?;
                error!(sl(), "Process exited, status: {:?}", exit_status);
            }
        };
        Ok(())
    }

    pub(crate) fn hypervisor_config(&self) -> HypervisorConfig {
        debug!(sl(), "[Firecracker]: Hypervisor config");
        self.config.clone()
    }

    pub(crate) fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        debug!(sl(), "[Firecracker]: Set Hypervisor config");
        self.config = config;
    }

    pub(crate) fn resize_memory(&mut self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        warn!(
            sl(),
            "memory size unchanged, requested: {:?} Not implemented", new_mem_mb
        );
        Ok((
            0,
            MemoryConfig {
                ..Default::default()
            },
        ))
    }

    pub(crate) fn set_capabilities(&mut self, flag: CapabilityBits) {
        self.capabilities.add(flag);
    }

    pub(crate) fn set_guest_memory_block_size(&mut self, size: u32) {
        warn!(
            sl(),
            "guest memory block size unchanged, requested: {:?}, Not implemented", size
        );
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        warn!(sl(), "guest memory block size Not implemented");
        0
    }
}

#[async_trait]
impl Persist for FcInner {
    type State = HypervisorState;
    type ConstructorArgs = ();

    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_FIRECRACKER.to_string(),
            id: self.id.clone(),
            vm_path: self.vm_path.clone(),
            config: self.hypervisor_config(),
            jailed: self.jailed,
            jailer_root: self.jailer_root.clone(),
            run_dir: self.run_dir.clone(),
            netns: self.netns.clone(),
            ..Default::default()
        })
    }
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        Ok(FcInner {
            id: hypervisor_state.id,
            asock_path: String::default(),
            state: VmmState::NotReady,
            vm_path: hypervisor_state.vm_path,
            config: hypervisor_state.config,
            netns: hypervisor_state.netns,
            pid: None,
            jailed: hypervisor_state.jailed,
            jailer_root: hypervisor_state.jailer_root,
            client: Client::unix(),
            pending_devices: vec![],
            run_dir: hypervisor_state.run_dir,
            capabilities: Capabilities::new(),
        })
    }
}
