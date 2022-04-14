// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod physical_endpoint;
pub use physical_endpoint::PhysicalEndpoint;
mod veth_endpoint;
pub use veth_endpoint::VethEndpoint;

use anyhow::Result;
use async_trait::async_trait;
use hypervisor::Hypervisor;

#[async_trait]
pub trait Endpoint: std::fmt::Debug + Send + Sync {
    async fn name(&self) -> String;
    async fn hardware_addr(&self) -> String;
    async fn attach(&self, hypervisor: &dyn Hypervisor) -> Result<()>;
    async fn detach(&self, hypervisor: &dyn Hypervisor) -> Result<()>;
}
