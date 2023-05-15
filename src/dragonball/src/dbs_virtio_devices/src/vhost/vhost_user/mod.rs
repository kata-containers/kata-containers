// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Vhost-based virtio device backend implementations.

#[cfg(feature = "vhost-user-blk")]
pub mod block;
pub mod connection;
#[cfg(feature = "vhost-user-fs")]
pub mod fs;

#[cfg(feature = "vhost-user-net")]
pub mod net;

#[cfg(test)]
mod test_utils;
