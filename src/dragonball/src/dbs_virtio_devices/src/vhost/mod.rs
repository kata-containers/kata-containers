// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Vhost-based virtio device backend implementations.

#[cfg(feature = "vhost-net")]
pub mod vhost_kern;

pub use vhost_rs::vhost_user::Error as VhostUserError;
pub use vhost_rs::Error as VhostError;

#[cfg(feature = "vhost-user")]
pub mod vhost_user;

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
