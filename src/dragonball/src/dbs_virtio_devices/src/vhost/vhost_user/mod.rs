// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Vhost-based virtio device backend implementations.

use super::VhostError;

pub mod connection;
#[cfg(feature = "vhost-user-fs")]
pub mod fs;

#[cfg(test)]
mod test_utils;
