// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Vhost-based virtio device backend implementations.

#[cfg(feature = "vhost-net")]
pub mod net;

#[cfg(all(feature = "vhost-net", test))]
pub mod test_utils;
