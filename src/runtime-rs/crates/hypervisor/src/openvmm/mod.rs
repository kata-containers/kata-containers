// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! OpenVMM hypervisor backend for kata-containers runtime-rs.
//!
//! This module integrates OpenVMM as an external, out-of-process hypervisor.
//! Kata launches the VMM and controls VM lifecycle over ttrpc while OpenVMM
//! uses the Microsoft Hypervisor backend on Azure Linux.

mod inner;
mod inner_device;
mod inner_hypervisor;
mod vmm_instance;

mod vmservice;
mod vmservice_ttrpc;

use ::protobuf::well_known_types::empty;

use inner::OpenVmmInner;
use persist::sandbox_persist::Persist;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::instrument;

use super::HypervisorState;
use crate::{DeviceType, Hypervisor, MemoryConfig, VcpuThreadIds};

// PCIe topology layout on the single root complex (bus 0). Every Kata device is
// a virtio (or vhost-user) function behind its own root port at function 0 of a
// distinct device number, so the guest-visible path is "DD/00". Device 0 (00.0)
// is intentionally left unused so the layout does not depend on whether the root
// complex reserves it. Cold-plug devices use fixed device numbers 1..=7; block
// hotplug ports use device numbers 8..=31 (hp0..hp23).
pub(crate) const OPENVMM_ROOTFS_PCI_DEVICE: u8 = 1;
pub(crate) const OPENVMM_SHAREFS_PCI_DEVICE: u8 = 2;
pub(crate) const OPENVMM_VSOCK_PCI_DEVICE: u8 = 3;
pub(crate) const OPENVMM_NET_PCI_FIRST_DEVICE: u8 = 4;
pub(crate) const OPENVMM_NET_PCI_MAX_COUNT: u8 = 4;
pub(crate) const OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE: u8 = 8;
pub(crate) const OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX: &str = "hp";
pub(crate) const OPENVMM_BLOCK_HOTPLUG_PORT_COUNT: u8 = 24;

/// The OpenVMM hypervisor struct, wrapping inner state behind a lock.
pub struct OpenVmm {
    inner: Arc<RwLock<OpenVmmInner>>,
    exit_waiter: Mutex<(mpsc::Receiver<i32>, i32)>,
}

impl std::fmt::Debug for OpenVmm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenVmm").finish()
    }
}

impl Default for OpenVmm {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenVmm {
    pub fn new() -> Self {
        let (exit_notify, exit_waiter) = mpsc::channel(1);

        Self {
            inner: Arc::new(RwLock::new(OpenVmmInner::new(exit_notify))),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        }
    }

    pub async fn set_hypervisor_config(&self, config: HypervisorConfig) {
        let mut inner = self.inner.write().await;
        inner.set_hypervisor_config(config);
    }
}

#[async_trait]
impl Hypervisor for OpenVmm {
    #[instrument]
    async fn prepare_vm(
        &self,
        id: &str,
        netns: Option<String>,
        _annotations: &HashMap<String, String>,
        _selinux_label: Option<String>,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_vm(id, netns).await
    }

    #[instrument]
    async fn start_vm(&self, timeout: i32) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.start_vm(timeout).await
    }

    async fn stop_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.stop_vm().await
    }

    async fn wait_vm(&self) -> Result<i32> {
        let mut waiter = self.exit_waiter.lock().await;
        if let Some(exit_code) = waiter.0.recv().await {
            waiter.1 = exit_code;
        }
        Ok(waiter.1)
    }

    async fn pause_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.pause_vm().await
    }

    async fn resume_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.resume_vm().await
    }

    async fn save_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.save_vm().await
    }

    async fn resize_vcpu(&self, old_vcpus: u32, new_vcpus: u32) -> Result<(u32, u32)> {
        let inner = self.inner.read().await;
        inner.resize_vcpu(old_vcpus, new_vcpus).await
    }

    async fn add_device(&self, device: DeviceType) -> Result<DeviceType> {
        let mut inner = self.inner.write().await;
        inner.add_device(device).await
    }

    async fn remove_device(&self, device: DeviceType) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.remove_device(device).await
    }

    async fn update_device(&self, device: DeviceType) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.update_device(device).await
    }

    async fn get_agent_socket(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_agent_socket().await
    }

    async fn disconnect(&self) {
        let mut inner = self.inner.write().await;
        inner.disconnect().await;
    }

    async fn hypervisor_config(&self) -> HypervisorConfig {
        let inner = self.inner.read().await;
        inner.hypervisor_config()
    }

    async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        let inner = self.inner.read().await;
        inner.get_thread_ids().await
    }

    async fn cleanup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.cleanup().await
    }

    async fn get_pids(&self) -> Result<Vec<u32>> {
        let inner = self.inner.read().await;
        inner.get_pids().await
    }

    async fn get_vmm_master_tid(&self) -> Result<u32> {
        let inner = self.inner.read().await;
        inner.get_vmm_master_tid().await
    }

    async fn get_ns_path(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_ns_path().await
    }

    async fn check(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.check().await
    }

    async fn get_jailer_root(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_jailer_root().await
    }

    async fn save_state(&self) -> Result<HypervisorState> {
        self.save().await
    }

    async fn capabilities(&self) -> Result<Capabilities> {
        let inner = self.inner.read().await;
        inner.capabilities().await
    }

    async fn get_hypervisor_metrics(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_hypervisor_metrics().await
    }

    async fn set_capabilities(&self, flag: CapabilityBits) {
        let mut inner = self.inner.write().await;
        inner.set_capabilities(flag);
    }

    async fn set_guest_memory_block_size(&self, size: u32) {
        let mut inner = self.inner.write().await;
        inner.set_guest_memory_block_size(size);
    }

    async fn guest_memory_block_size(&self) -> u32 {
        let inner = self.inner.read().await;
        inner.guest_memory_block_size_mb()
    }

    async fn resize_memory(&self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        let mut inner = self.inner.write().await;
        inner.resize_memory(new_mem_mb).await
    }

    async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        let inner = self.inner.read().await;
        inner.get_passfd_listener_addr().await
    }
}

#[async_trait]
impl Persist for OpenVmm {
    type State = HypervisorState;
    type ConstructorArgs = ();

    async fn save(&self) -> Result<Self::State> {
        let inner = self.inner.read().await;
        inner.save().await.context("save openvmm hypervisor state")
    }

    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let (exit_notify, exit_waiter) = mpsc::channel(1);
        let inner = OpenVmmInner::restore(exit_notify, hypervisor_state).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        })
    }
}
