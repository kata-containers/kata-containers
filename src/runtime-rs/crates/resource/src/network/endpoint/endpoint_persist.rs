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
/// consumer there; an endpoint that leaves nothing on the host for recovery
/// to act on records nothing.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EndpointState {
    #[serde(default)]
    pub physical_endpoint: Option<PhysicalEndpointState>,
    /// The TAP interface the endpoint's constructor made in the network
    /// namespace, which exists whether or not the attach ever ran.
    #[serde(default)]
    pub tap_iface_name: Option<String>,
    /// The endpoint was constructed but its attach never started: nothing
    /// reached the hypervisor or left the namespace, and only the
    /// construction artifacts are there to remove. States saved before this
    /// field existed only recorded endpoints whose attach had started.
    #[serde(default)]
    pub construction_only: bool,
}
