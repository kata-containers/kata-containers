// Copyright (C) 2019-2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! API Version 1 related data structures to configure the vmm.

mod vmm_action;
pub use self::vmm_action::*;

/// Wrapper for configuring the microVM boot source.
mod boot_source;
pub use self::boot_source::{BootSourceConfig, BootSourceConfigError, DEFAULT_KERNEL_CMDLINE};

/// Wrapper over the microVM general information.
mod instance_info;
pub use self::instance_info::{ConfidentialVmType, InstanceInfo, InstanceState};

/// Wrapper for configuring the memory and CPU of the microVM.
mod machine_config;
pub use self::machine_config::{VmConfigError, MAX_SUPPORTED_VCPUS};

/// Wrapper for configuring the virtio networking
#[cfg(any(
    feature = "virtio-net",
    feature = "vhost-net",
    feature = "vhost-user-net"
))]
#[cfg(feature = "vhost-user-net")]
pub use crate::device_manager::net_dev_mgr::VhostUserConfig;
#[cfg(any(feature = "virtio-net", feature = "vhost-net"))]
pub use crate::device_manager::net_dev_mgr::VirtioConfig;
#[cfg(any(
    feature = "virtio-net",
    feature = "vhost-net",
    feature = "vhost-user-net"
))]
pub use crate::device_manager::net_dev_mgr::{
    Backend, NetworkInterfaceConfig, NetworkInterfaceUpdateConfig,
};
