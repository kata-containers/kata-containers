// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use crate::NetworkAllocation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DanEndpoint {
    HostTap { tap_name: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DanDevice {
    pub guest_if_name: String,
    pub endpoint: DanEndpoint,
    pub allocation: NetworkAllocation,
}
