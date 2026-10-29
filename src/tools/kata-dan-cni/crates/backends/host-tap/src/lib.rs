// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

mod backend;
mod naming;
mod state;
mod tap;

pub use backend::HostTapBackend;

pub const BACKEND_NAME: &str = "host-tap";

const OVS_BRIDGE_NAME: &str = "br-int";
