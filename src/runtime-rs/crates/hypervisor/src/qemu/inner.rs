// Copyright (c) 2022 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use super::cmdline_generator::QemuCmdLine;
use crate::{
    hypervisor_persist::HypervisorState, HypervisorConfig, MemoryConfig, VcpuThreadIds,
    VsockDevice, HYPERVISOR_QEMU,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use kata_types::{
    capabilities::{Capabilities, CapabilityBits},
    config::KATA_PATH,
};
use persist::sandbox_persist::Persist;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::process::Child;

const VSOCK_SCHEME: &str = "vsock";

#[derive(Debug)]
pub struct QemuInner {
    /// sandbox id
    id: String,

    qemu_process: Option<Child>,

    config: HypervisorConfig,
    devices: Vec<DeviceType>,
}

impl QemuInner {
    pub fn new() -> QemuInner {
        QemuInner {
            id: "".to_string(),
            qemu_process: None,
            config: Default::default(),
            devices: Vec::new(),
        }
    }

    pub(crate) async fn prepare_vm(&mut self, id: &str, _netns: Option<String>) -> Result<()> {
        info!(sl!(), "Preparing QEMU VM");
        self.id = id.to_string();

        let vm_path = [KATA_PATH, self.id.as_str()].join("/");
        std::fs::create_dir_all(vm_path)?;

        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, _timeout: i32) -> Result<()> {
        info!(sl!(), "Starting QEMU VM");

        let mut cmdline = QemuCmdLine::new(&self.id, &self.config)?;

        // If there's a Vsock Device in `self.devices` the vhost-vsock file
        // descriptor needs to stay open until the qemu process launches.
        // This is why we need to store it in a variable at this scope.
        let mut _vhost_fd = None;

        for device in &mut self.devices {
            match device {
                DeviceType::ShareFs(share_fs_dev) => {
                    if share_fs_dev.config.fs_type == "virtio-fs" {
                        cmdline.add_virtiofs_share(
                            &share_fs_dev.config.sock_path,
                            &share_fs_dev.config.mount_tag,
                            share_fs_dev.config.queue_size,
                        );
                    }
                }
                DeviceType::Vsock(vsock_dev) => {
                    let fd = vsock_dev.init_config().await?;
                    cmdline.add_vsock(fd.as_raw_fd(), vsock_dev.config.guest_cid)?;
                    _vhost_fd = Some(fd);
                }
                DeviceType::Block(block_dev) => {
                    if block_dev.config.path_on_host == self.config.boot_info.initrd {
                        // If this block device represents initrd we ignore it here, it
                        // will be handled elsewhere by adding `-initrd` to the qemu
                        // command line.
                        continue;
                    }
                    match block_dev.config.driver_option.as_str() {
                        "nvdimm" => cmdline.add_nvdimm(
                            &block_dev.config.path_on_host,
                            block_dev.config.is_readonly,
                        )?,
                        unsupported => {
                            info!(sl!(), "unsupported block device driver: {}", unsupported)
                        }
                    }
                }
                _ => info!(sl!(), "qemu cmdline: unsupported device: {:?}", device),
            }
        }
        // To get access to the VM console for debugging, enable the following
        // line and replace its argument appropriately (open a terminal, run
        // `tty` in it to get its device file path and use it as the argument).
        //cmdline.add_serial_console("/dev/pts/23");

        info!(sl!(), "qemu args: {}", cmdline.build().await?.join(" "));
        let mut command = std::process::Command::new(&self.config.path);
        command.args(cmdline.build().await?);

        info!(sl!(), "qemu cmd: {:?}", command);
        self.qemu_process = Some(command.spawn()?);

        Ok(())
    }

    pub(crate) fn stop_vm(&mut self) -> Result<()> {
        info!(sl!(), "Stopping QEMU VM");
        if let Some(ref mut qemu_process) = &mut self.qemu_process {
            info!(sl!(), "QemuInner::stop_vm(): kill()'ing qemu");
            qemu_process.kill().map_err(anyhow::Error::from)
        } else {
            Err(anyhow!("qemu process not running"))
        }
    }

    pub(crate) fn pause_vm(&self) -> Result<()> {
        info!(sl!(), "Pausing QEMU VM");
        todo!()
    }

    pub(crate) fn resume_vm(&self) -> Result<()> {
        info!(sl!(), "Resuming QEMU VM");
        todo!()
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        todo!()
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        info!(sl!(), "QemuInner::get_agent_socket()");
        let guest_cid = match &self.get_agent_vsock_dev() {
            Some(device) => device.config.guest_cid,
            None => return Err(anyhow!("uninitialized agent vsock".to_owned())),
        };

        Ok(format!("{}://{}", VSOCK_SCHEME, guest_cid))
    }

    pub(crate) async fn disconnect(&mut self) {
        info!(sl!(), "QemuInner::disconnect()");
        todo!()
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        info!(sl!(), "QemuInner::get_thread_ids()");
        //todo!()
        let vcpu_thread_ids: VcpuThreadIds = VcpuThreadIds {
            vcpus: HashMap::new(),
        };
        Ok(vcpu_thread_ids)
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        info!(sl!(), "QemuInner::get_vmm_master_tid()");
        if let Some(qemu_process) = &self.qemu_process {
            info!(
                sl!(),
                "QemuInner::get_vmm_master_tid(): returning {}",
                qemu_process.id()
            );
            Ok(qemu_process.id())
        } else {
            Err(anyhow!("qemu process not running"))
        }
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        info!(sl!(), "QemuInner::get_ns_path()");
        Ok(format!(
            "/proc/{}/task/{}/ns",
            std::process::id(),
            std::process::id()
        ))
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        info!(sl!(), "QemuInner::cleanup()");
        let vm_path = [KATA_PATH, self.id.as_str()].join("/");
        std::fs::remove_dir_all(vm_path)?;
        Ok(())
    }

    pub(crate) async fn resize_vcpu(&self, old_vcpus: u32, new_vcpus: u32) -> Result<(u32, u32)> {
        info!(
            sl!(),
            "QemuInner::resize_vcpu(): {} -> {}", old_vcpus, new_vcpus
        );
        if new_vcpus == old_vcpus {
            return Ok((old_vcpus, new_vcpus));
        }
        todo!()
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        info!(sl!(), "QemuInner::get_pids()");
        todo!()
    }

    pub(crate) async fn check(&self) -> Result<()> {
        todo!()
    }

    pub(crate) async fn get_jailer_root(&self) -> Result<String> {
        Ok("".into())
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        let mut caps = Capabilities::default();
        caps.set(CapabilityBits::FsSharingSupport);
        Ok(caps)
    }

    pub fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = config;
    }

    pub fn hypervisor_config(&self) -> HypervisorConfig {
        info!(sl!(), "QemuInner::hypervisor_config()");
        self.config.clone()
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        todo!()
    }

    pub(crate) fn set_capabilities(&mut self, _flag: CapabilityBits) {
        todo!()
    }

    pub(crate) fn set_guest_memory_block_size(&mut self, _size: u32) {
        warn!(
            sl!(),
            "QemuInner::set_guest_memory_block_size(): NOT YET IMPLEMENTED"
        );
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        warn!(
            sl!(),
            "QemuInner::guest_memory_block_size_mb(): NOT YET IMPLEMENTED"
        );
        0
    }

    pub(crate) fn resize_memory(&self, _new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        warn!(sl!(), "QemuInner::resize_memory(): NOT YET IMPLEMENTED");
        Ok((
            _new_mem_mb,
            MemoryConfig {
                ..Default::default()
            },
        ))
    }
}

use crate::device::DeviceType;

// device manager part of Hypervisor
impl QemuInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        info!(sl!(), "QemuInner::add_device() {}", device);
        self.devices.push(device.clone());
        Ok(device)
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "QemuInner::remove_device() {} ", device);
        Err(anyhow!(
            "QemuInner::remove_device({}): Not yet implemented",
            device
        ))
    }
}

// private helpers
impl QemuInner {
    fn get_agent_vsock_dev(&self) -> Option<&VsockDevice> {
        self.devices.iter().find_map(|dev| {
            if let DeviceType::Vsock(vsock_dev) = dev {
                Some(vsock_dev)
            } else {
                None
            }
        })
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "QemuInner::update_device() {:?}", &device);

        Ok(())
    }
}

#[async_trait]
impl Persist for QemuInner {
    type State = HypervisorState;
    type ConstructorArgs = ();

    /// Save a state of hypervisor
    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_QEMU.to_string(),
            id: self.id.clone(),
            config: self.hypervisor_config(),
            ..Default::default()
        })
    }

    /// Restore hypervisor
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        Ok(QemuInner {
            id: hypervisor_state.id,
            qemu_process: None,
            config: hypervisor_state.config,
            devices: Vec::new(),
        })
    }
}
