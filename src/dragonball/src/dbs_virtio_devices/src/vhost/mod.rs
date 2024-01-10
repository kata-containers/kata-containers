// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Vhost-based virtio device backend implementations.

#[cfg(feature = "vhost")]
pub mod vhost_kern;

#[cfg(feature = "vhost-user")]
pub mod vhost_user;

/// Common code for vhost-based network device
#[cfg(any(feature = "vhost-net", feature = "vhost-user-net"))]
mod net;

pub use vhost_rs::vhost_user::Error as VhostUserError;
pub use vhost_rs::Error as VhostError;

impl std::convert::From<VhostError> for super::Error {
    fn from(e: VhostError) -> Self {
        super::Error::VhostError(e)
    }
}

impl std::convert::From<VhostUserError> for super::Error {
    fn from(e: VhostUserError) -> Self {
        super::Error::VhostUserError(e)
    }
}
