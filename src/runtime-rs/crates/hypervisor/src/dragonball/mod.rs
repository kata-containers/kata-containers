// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod inner;
mod inner_device;
mod inner_hypervisor;
use super::HypervisorState;
use inner::DragonballInner;
use persist::sandbox_persist::Persist;
pub mod vmm_instance;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use dbs_utils::net::MacAddr as DragonballMacAddr;
use dragonball::api::v1::{
    Backend as DragonballBackend, NetworkInterfaceConfig as DragonballNetworkConfig,
    VirtioConfig as DragonballVirtioConfig,
};
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::instrument;

use crate::{DeviceType, Hypervisor, MemoryConfig, NetworkConfig, VcpuThreadIds};

pub struct Dragonball {
    inner: Arc<RwLock<DragonballInner>>,
    exit_waiter: Mutex<(mpsc::Receiver<i32>, i32)>,
}

impl std::fmt::Debug for Dragonball {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dragonball").finish()
    }
}

impl Default for Dragonball {
    fn default() -> Self {
        Self::new()
    }
}

impl Dragonball {
    pub fn new() -> Self {
        let (exit_notify, exit_waiter) = mpsc::channel(1);

        Self {
            inner: Arc::new(RwLock::new(DragonballInner::new(exit_notify))),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        }
    }

    pub async fn set_hypervisor_config(&self, config: HypervisorConfig) {
        let mut inner = self.inner.write().await;
        inner.set_hypervisor_config(config)
    }

    pub async fn set_passfd_listener_port(&self, port: u32) {
        let mut inner = self.inner.write().await;
        inner.set_passfd_listener_port(port)
    }
}

#[async_trait]
impl Hypervisor for Dragonball {
    #[instrument]
    async fn prepare_vm(
        &self,
        id: &str,
        netns: Option<String>,
        _annotations: &HashMap<String, String>,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_vm(id, netns).await
    }

    #[instrument]
    async fn start_vm(&self, timeout: i32) -> Result<()> {
        let mut inner = self.inner.write().await;
        let ret = inner.start_vm(timeout).await;

        if ret.is_ok() && inner.config.device_info.reclaim_guest_freed_memory {
            // The virtio-balloon device must be inserted into dragonball and
            // recognized by the guest kernel only after the dragonball upcall is ready.
            // The dragonball upcall is not ready immediately after the VM starts,
            // so here we create an asynchronous task that waits for 5 seconds before
            // inserting the virtio-balloon device.
            let inner_clone = self.inner.clone();
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                inner_clone
                    .write()
                    .await
                    .try_insert_balloon_f_reporting()
                    .await;
            });
        }

        ret
    }

    async fn stop_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.stop_vm()
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
        inner.pause_vm()
    }

    async fn resume_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.resume_vm()
    }

    async fn save_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.save_vm().await
    }

    // returns Result<(old_vcpus, new_vcpus)>
    async fn resize_vcpu(&self, old_vcpus: u32, new_vcpus: u32) -> Result<(u32, u32)> {
        let inner = self.inner.read().await;
        inner.resize_vcpu(old_vcpus, new_vcpus).await
    }

    async fn add_device(&self, device: DeviceType) -> Result<DeviceType> {
        let mut inner = self.inner.write().await;
        inner.add_device(device.clone()).await
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
        inner.disconnect().await
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
        inner.set_capabilities(flag)
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
        inner.resize_memory(new_mem_mb)
    }

    async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        let inner = self.inner.read().await;
        inner.get_passfd_listener_addr().await
    }
}

#[async_trait]
impl Persist for Dragonball {
    type State = HypervisorState;
    type ConstructorArgs = ();
    /// Save a state of the component.
    async fn save(&self) -> Result<Self::State> {
        let inner = self.inner.read().await;
        inner.save().await.context("save hypervisor state")
    }
    /// Restore a component from a specified state.
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let (exit_notify, exit_waiter) = mpsc::channel(1);

        let inner = DragonballInner::restore(exit_notify, hypervisor_state).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        })
    }
}

/// Generate Dragonball network config according to hypervisor config and
/// runtime network config.
pub(crate) fn build_dragonball_network_config(
    hconfig: &HypervisorConfig,
    nconfig: &NetworkConfig,
) -> DragonballNetworkConfig {
    let virtio_config = DragonballVirtioConfig {
        iface_id: nconfig.virt_iface_name.clone(),
        host_dev_name: nconfig.host_dev_name.clone(),
        // TODO(justxuewei): rx_rate_limiter is not supported, see:
        // https://github.com/kata-containers/kata-containers/issues/8327.
        rx_rate_limiter: None,
        // TODO(justxuewei): tx_rate_limiter is not supported, see:
        // https://github.com/kata-containers/kata-containers/issues/8327.
        tx_rate_limiter: None,
        allow_duplicate_mac: nconfig.allow_duplicate_mac,
    };

    let backend = if hconfig.network_info.disable_vhost_net {
        DragonballBackend::Virtio(virtio_config)
    } else {
        DragonballBackend::Vhost(virtio_config)
    };

    DragonballNetworkConfig {
        num_queues: Some(nconfig.queue_num),
        queue_size: Some(nconfig.queue_size as u16),
        backend,
        guest_mac: nconfig.guest_mac.clone().map(|mac| {
            // We are safety since mac address is checked by endpoints.
            DragonballMacAddr::from_bytes(&mac.0).unwrap()
        }),
        use_shared_irq: nconfig.use_shared_irq,
        use_generic_irq: nconfig.use_generic_irq,
    }
}
