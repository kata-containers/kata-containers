// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};

/// What recovery takes to give a passed-through device back to its host
/// driver: the address to act on and the driver to bind. Nothing else of a
/// physical endpoint is recorded.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PhysicalEndpointState {
    pub bdf: String,
    pub driver: String,
}

/// An endpoint, as the recovery of a dead shim needs it. Every field has a
/// consumer there, and TAPs need none: recovery removes them from the
/// recorded namespace by the name suffix every Kata link is created with,
/// so an endpoint that passes no device through records nothing.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EndpointState {
    #[serde(default)]
    pub physical_endpoint: Option<PhysicalEndpointState>,
}
