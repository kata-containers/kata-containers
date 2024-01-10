// Copyright (c) 2022 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;

use crate::{HypervisorConfig, MemoryConfig, VcpuThreadIds};
use kata_types::capabilities::{Capabilities, CapabilityBits};

const VSOCK_SCHEME: &str = "vsock";
const VSOCK_AGENT_CID: u32 = 3;
const VSOCK_AGENT_PORT: u32 = 1024;
#[derive(Debug)]
pub struct QemuInner {
    config: HypervisorConfig,
}

impl QemuInner {
    pub fn new() -> QemuInner {
        QemuInner {
            config: Default::default(),
        }
    }

    pub(crate) async fn prepare_vm(&mut self, _id: &str, _netns: Option<String>) -> Result<()> {
        info!(sl!(), "Preparing QEMU VM");
        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, _timeout: i32) -> Result<()> {
        info!(sl!(), "Starting QEMU VM");

        let mut command = std::process::Command::new(&self.config.path);

        command
            .arg("-kernel")
            .arg(&self.config.boot_info.kernel)
            .arg("-m")
            .arg(format!("{}M", &self.config.memory_info.default_memory))
            .arg("-initrd")
            .arg(&self.config.boot_info.initrd)
            .arg("-vga")
            .arg("none")
            .arg("-nodefaults")
            .arg("-nographic");

        command.spawn()?;

        Ok(())
    }

    pub(crate) fn stop_vm(&mut self) -> Result<()> {
        info!(sl!(), "Stopping QEMU VM");
        todo!()
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

    /// TODO: using a single hardcoded CID is clearly not adequate in the long
    /// run. Use the recently added VsockConfig infrastructure to fix this.
    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        info!(sl!(), "QemuInner::get_agent_socket()");
        Ok(format!(
            "{}://{}:{}",
            VSOCK_SCHEME, VSOCK_AGENT_CID, VSOCK_AGENT_PORT
        ))
    }

    pub(crate) async fn disconnect(&mut self) {
        info!(sl!(), "QemuInner::disconnect()");
        todo!()
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        info!(sl!(), "QemuInner::get_thread_ids()");
        todo!()
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        info!(sl!(), "QemuInner::get_vmm_master_tid()");
        todo!()
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        info!(sl!(), "QemuInner::get_ns_path()");
        todo!()
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        info!(sl!(), "QemuInner::cleanup()");
        todo!()
    }

    pub(crate) async fn resize_vcpu(&self, _old_vcpus: u32, _new_vcpus: u32) -> Result<(u32, u32)> {
        info!(sl!(), "QemuInner::resize_vcpu()");
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
        todo!()
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
        todo!()
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        todo!()
    }

    pub(crate) fn resize_memory(&self, _new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        todo!()
    }
}

use crate::device::DeviceType;

// device manager part of Hypervisor
impl QemuInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        info!(sl!(), "QemuInner::add_device() {}", device);
        Ok(device)
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "QemuInner::remove_device() {} ", device);
        todo!()
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "QemuInner::update_device() {:?}", &device);

        Ok(())
    }
}
