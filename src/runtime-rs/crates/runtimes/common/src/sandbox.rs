// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Sandbox: Send + Sync {
    // sandbox lifetime management
    async fn start(&self, netns: Option<String>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn cleanup(&self, container_id: &str) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;

    // agent function
    async fn agent_sock(&self) -> Result<String>;

    // sandbox resource management
    async fn update_cpu_resource(&self, new_vcpus: u32) -> Result<()>;
}
