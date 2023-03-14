// Copyright 2021 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(feature = "fusedev", target_os = "linux"))]
pub(crate) mod passthroughfs;

#[cfg(all(feature = "fusedev", target_os = "macos"))]
pub(crate) mod macfuse;
