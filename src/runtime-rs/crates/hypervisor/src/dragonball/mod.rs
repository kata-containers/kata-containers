// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod inner;
mod inner_device;
mod inner_hypervisor;
use inner::DragonballInner;
pub mod vmm_instance;

pub const RUN_PATH_PREFIX: &str = "/run/kata";

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use tokio::sync::RwLock;

use crate::{device::Device, Hypervisor, VcpuThreadIds};

unsafe impl Send for Dragonball {}
unsafe impl Sync for Dragonball {}
pub struct Dragonball {
    inner: Arc<RwLock<DragonballInner>>,
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
    async fn prepare_vm(&self, id: &str, netns: Option<String>) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_vm(id, netns).await
    }

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

    async fn add_device(&self, device: Device) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.add_device(device).await
    }

    async fn remove_device(&self, device: Device) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.remove_device(device).await
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

    async fn check(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.check().await
    }

    async fn get_jailer_root(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_jailer_root().await
    }
}
