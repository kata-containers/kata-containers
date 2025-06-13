// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate slog;

logging::logger_with_subsystem!(sl, "resource");

pub mod cgroups;
pub mod manager;
mod manager_inner;
pub mod network;
pub mod resource_persist;
use hypervisor::{
    BlockConfig, HybridVsockConfig, PortDeviceConfig, ProtectionDeviceConfig, VsockConfig,
};
use network::NetworkConfig;
pub mod rootfs;
pub mod share_fs;
pub mod volume;
pub use manager::ResourceManager;
pub mod cdi_devices;
pub mod coco_data;
pub mod cpu_mem;

use kata_types::config::hypervisor::SharedFsInfo;

#[derive(Debug)]
pub enum ResourceConfig {
    Network(NetworkConfig),
    ShareFs(SharedFsInfo),
    VmRootfs(BlockConfig),
    HybridVsock(HybridVsockConfig),
    Vsock(VsockConfig),
    Protection(ProtectionDeviceConfig),
    PortDevice(PortDeviceConfig),
    InitData(BlockConfig),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResourceUpdateOp {
    Add,
    Del,
    Update,
}
