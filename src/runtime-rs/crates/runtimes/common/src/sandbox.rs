// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;
use oci_spec::runtime as oci;
use runtime_spec as spec;

#[derive(Clone)]
pub struct SandboxNetworkEnv {
    pub netns: Option<String>,
    pub network_created: bool,
}

impl std::fmt::Debug for SandboxNetworkEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxNetworkEnv")
            .field("netns", &self.netns)
            .field("network_created", &self.network_created)
            .finish()
    }
}

#[async_trait]
pub trait Sandbox: Send + Sync {
    async fn start(
        &self,
        dns: Vec<String>,
        spec: &oci::Spec,
        state: &spec::State,
        network_env: SandboxNetworkEnv,
    ) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn cleanup(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;

    // utils
    async fn set_iptables(&self, is_ipv6: bool, data: Vec<u8>) -> Result<Vec<u8>>;
    async fn get_iptables(&self, is_ipv6: bool) -> Result<Vec<u8>>;
    async fn direct_volume_stats(&self, volume_path: &str) -> Result<String>;
    async fn direct_volume_resize(&self, resize_req: agent::ResizeVolumeRequest) -> Result<()>;
    async fn agent_sock(&self) -> Result<String>;

    // metrics function
    async fn agent_metrics(&self) -> Result<String>;
    async fn hypervisor_metrics(&self) -> Result<String>;
}
