// Copyright (C) 2022 Alibaba Cloud Computing. All rights reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

mod sm;
mod vcpu_impl;
mod vcpu_manager;

use dbs_arch::VpmuFeatureLevel;
pub use vcpu_manager::{VcpuManager, VcpuManagerError, VcpuResizeInfo};

#[cfg(feature = "hotplug")]
pub use vcpu_manager::VcpuResizeError;

/// vcpu config collection
pub struct VcpuConfig {
    /// initial vcpu count
    pub boot_vcpu_count: u8,
    /// max vcpu count for hotplug
    pub max_vcpu_count: u8,
    /// threads per core for cpu topology information
    pub threads_per_core: u8,
    /// cores per die for cpu topology information
    pub cores_per_die: u8,
    /// dies per socket for cpu topology information
    pub dies_per_socket: u8,
    /// socket number for cpu topology information
    pub sockets: u8,
    /// if vpmu feature is Disabled, it means vpmu feature is off (by default)
    /// if vpmu feature is LimitedlyEnabled, it means minimal vpmu counters are supported (cycles and instructions)
    /// if vpmu feature is FullyEnabled, it means all vpmu counters are supported
    /// For aarch64, VpmuFeatureLevel only supports Disabled and FullyEnabled.
    pub vpmu_feature: VpmuFeatureLevel,
}
