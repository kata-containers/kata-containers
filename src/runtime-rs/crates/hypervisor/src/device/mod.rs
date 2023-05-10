// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{DeviceConfig, Hypervisor as hypervisor};
use anyhow::Result;
use async_trait::async_trait;

pub mod device_manager;
pub mod driver;
pub mod util;

#[async_trait]
pub trait Device: Send + Sync {
    // attach is to plug device into VM
    async fn attach(&mut self, h: &dyn hypervisor) -> Result<()>;
    // detach is to unplug device from VM
    async fn detach(&mut self, h: &dyn hypervisor) -> Result<Option<u64>>;
    // get_device_info returns device config
    async fn get_device_info(&self) -> DeviceConfig;
    // increase_attach_count is used to increase the attach count for a device
    // return values:
    // * true: no need to do real attach when current attach count is zero, skip following actions.
    // * err error: error while do increase attach count
    async fn increase_attach_count(&mut self) -> Result<bool>;
    // decrease_attach_count is used to decrease the attach count for a device
    // return values:
    // * false: no need to do real dettach when current attach count is not zero, skip following actions.
    // * err error: error while do decrease attach count
    async fn decrease_attach_count(&mut self) -> Result<bool>;
}
