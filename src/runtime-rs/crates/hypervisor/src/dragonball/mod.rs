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

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use dbs_utils::net::MacAddr as DragonballMacAddr;
use dragonball::api::v1::{
    Backend as DragonballBackend, NetworkInterfaceConfig as DragonballNetworkConfig,
    VirtioConfig as DragonballVirtioConfig,
};
use kata_types::capabilities::Capabilities;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::{DeviceType, Hypervisor, NetworkConfig, VcpuThreadIds};

pub struct Dragonball {
    inner: Arc<RwLock<DragonballInner>>,
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
        Self {
            inner: Arc::new(RwLock::new(DragonballInner::new())),
        }
    }

    pub async fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        let mut inner = self.inner.write().await;
        inner.set_hypervisor_config(config)
    }
}

#[async_trait]
impl Hypervisor for Dragonball {
    #[instrument]
    async fn prepare_vm(&self, id: &str, netns: Option<String>) -> Result<()> {
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
        inner.stop_vm()
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
        match inner.add_device(device.clone()).await {
            Ok(_) => Ok(device),
            Err(err) => Err(err),
        }
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
        hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let inner = DragonballInner::restore(hypervisor_args, hypervisor_state).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
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
