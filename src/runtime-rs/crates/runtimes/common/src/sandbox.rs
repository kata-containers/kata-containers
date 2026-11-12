// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    types::{ContainerProcess, SandboxExitInfo, SandboxStatus},
    ContainerManager,
};

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct SandboxNetworkEnv {
    pub netns: Option<String>,
    pub network_created: bool,
}

impl SandboxNetworkEnv {
    /// The network namespace the runtime created for the sandbox, which it
    /// has to remove again. A namespace handed in by the caller is theirs.
    pub fn created_netns(&self) -> Option<String> {
        if self.network_created {
            self.netns.clone()
        } else {
            None
        }
    }
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
    async fn start(&self) -> Result<()>;
    async fn start_template(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn cleanup(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
    async fn status(&self) -> Result<SandboxStatus>;
    async fn wait(&self) -> Result<SandboxExitInfo>;
    // Record what a later recovery has to clean up if this shim goes away.
    async fn save_state(&self) -> Result<()>;
    // Record the host artifacts creating a container from `spec` will make,
    // before any of them exists, so a shim killed during the create leaves a
    // state that already covers them.
    async fn save_container_intents(&self, spec: &oci_spec::runtime::Spec) -> Result<()>;

    // utils
    async fn set_iptables(&self, is_ipv6: bool, data: Vec<u8>) -> Result<Vec<u8>>;
    async fn get_iptables(&self, is_ipv6: bool) -> Result<Vec<u8>>;
    async fn direct_volume_stats(&self, volume_path: &str) -> Result<String>;
    async fn direct_volume_resize(&self, resize_req: agent::ResizeVolumeRequest) -> Result<()>;
    async fn agent_sock(&self) -> Result<String>;
    async fn wait_process(
        &self,
        cm: Arc<dyn ContainerManager>,
        process_id: ContainerProcess,
        shim_pid: u32,
    ) -> Result<()>;
    // Returns once wait_process has sent TaskExit for the sandbox container,
    // or at once if nothing is waiting for that exit.
    async fn wait_exit_published(&self);

    // Docker 26+ network rescan: discover interfaces that Docker configured
    // between the Create and Start RPCs.
    async fn rescan_network(&self) -> Result<()>;

    // metrics function
    async fn agent_metrics(&self) -> Result<String>;
    async fn hypervisor_metrics(&self) -> Result<String>;

    // set agent policy
    async fn set_policy(&self, policy: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::created(Some("/run/netns/cnitest"), true, Some("/run/netns/cnitest"))]
    #[case::given_by_the_caller(Some("/var/run/netns/cni-1"), false, None)]
    #[case::none(None, true, None)]
    fn only_a_netns_the_runtime_created_is_recorded_for_removal(
        #[case] netns: Option<&str>,
        #[case] network_created: bool,
        #[case] expected: Option<&str>,
    ) {
        let network_env = SandboxNetworkEnv {
            netns: netns.map(String::from),
            network_created,
        };
        assert_eq!(network_env.created_netns().as_deref(), expected);
    }
}
