// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Fuse Application Binary Interfaces(ABI).

/// Linux/Macos Fuse Application Binary Interfaces.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod fuse_abi;

#[cfg(feature = "virtiofs")]
pub mod virtio_fs;
