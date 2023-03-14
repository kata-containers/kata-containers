// Copyright 2019 Red Hat, Inc. All Rights Reserved.
// SPDX-License-Identifier: (BSD-3-Clause OR Apache-2.0)

#[cfg(feature = "virtio-v4_14_0")]
mod bindings_v4_14_0;
#[cfg(feature = "virtio-v5_0_0")]
mod bindings_v5_0_0;

// Major hack to have a default version in case no feature is specified:
// If no version is specified by using the features, just use the latest one
// which currently is 5.0.
#[cfg(all(not(feature = "virtio-v4_14_0"), not(feature = "virtio-v5_0_0")))]
mod bindings_v5_0_0;

pub mod bindings {
    #[cfg(feature = "virtio-v4_14_0")]
    pub use super::bindings_v4_14_0::*;

    #[cfg(feature = "virtio-v5_0_0")]
    pub use super::bindings_v5_0_0::*;

    #[cfg(all(not(feature = "virtio-v4_14_0"), not(feature = "virtio-v5_0_0")))]
    pub use super::bindings_v5_0_0::*;
}
