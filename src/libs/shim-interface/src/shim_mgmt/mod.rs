// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

/// The shim management client module
pub mod client;

/// The key for direct volume path
pub const DIRECT_VOLUME_PATH_KEY: &str = "path";
/// URL for stats direct volume
pub const DIRECT_VOLUME_STATS_URL: &str = "/direct-volume/stats";
/// URL for resizing direct volume
pub const DIRECT_VOLUME_RESIZE_URL: &str = "/direct-volume/resize";
/// URL for querying agent's socket
pub const AGENT_URL: &str = "/agent-url";
/// URL for operation on guest iptable (ipv4)
pub const IP_TABLE_URL: &str = "/iptables";
/// URL for operation on guest iptable (ipv6)
pub const IP6_TABLE_URL: &str = "/ip6tables";
/// URL for querying metrics inside shim
pub const METRICS_URL: &str = "/metrics";

pub const ERR_NO_SHIM_SERVER: &str = "Failed to create shim management server";
