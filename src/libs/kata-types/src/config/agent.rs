// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;

use crate::config::{ConfigOps, TomlConfig};

pub use vendor::AgentVendor;

use super::default::{
    DEFAULT_AGENT_DIAL_TIMEOUT_MS, DEFAULT_AGENT_LOG_PORT, DEFAULT_AGENT_VSOCK_PORT,
    DEFAULT_PASSFD_LISTENER_PORT,
};
use crate::eother;

/// agent name of Kata agent.
pub const AGENT_NAME_KATA: &str = "kata";

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
pub struct MemAgent {
    #[serde(default, alias = "mem_agent_enable")]
    pub enable: bool,

    #[serde(default)]
    pub memcg_disable: Option<bool>,
    #[serde(default)]
    pub memcg_swap: Option<bool>,
    #[serde(default)]
    pub memcg_swappiness_max: Option<u8>,
    #[serde(default)]
    pub memcg_period_secs: Option<u64>,
    #[serde(default)]
    pub memcg_period_psi_percent_limit: Option<u8>,
    #[serde(default)]
    pub memcg_eviction_psi_percent_limit: Option<u8>,
    #[serde(default)]
    pub memcg_eviction_run_aging_count_min: Option<u64>,

    #[serde(default)]
    pub compact_disable: Option<bool>,
    #[serde(default)]
    pub compact_period_secs: Option<u64>,
    #[serde(default)]
    pub compact_period_psi_percent_limit: Option<u8>,
    #[serde(default)]
    pub compact_psi_percent_limit: Option<u8>,
    #[serde(default)]
    pub compact_sec_max: Option<i64>,
    #[serde(default)]
    pub compact_order: Option<u8>,
    #[serde(default)]
    pub compact_threshold: Option<u64>,
    #[serde(default)]
    pub compact_force_times: Option<u64>,
}

/// Kata agent configuration information.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Agent {
    /// If enabled, the agent will log additional debug messages to the system log.
    #[serde(default, rename = "enable_debug")]
    pub debug: bool,

    /// The log log level will be applied to agent.
    /// Possible values are:
    /// - trace
    /// - debug
    /// - info
    /// - warn
    /// - error
    /// - critical
    #[serde(default = "default_agent_log_level")]
    pub log_level: String,

    /// Enable agent tracing.
    ///
    /// If enabled, the agent will generate OpenTelemetry trace spans.
    /// # Notes:
    /// - If the runtime also has tracing enabled, the agent spans will be associated with the
    ///   appropriate runtime parent span.
    /// - If enabled, the runtime will wait for the container to shutdown, increasing the container
    ///   shutdown time slightly.
    #[serde(default)]
    pub enable_tracing: bool,

    /// Enable debug console.
    /// If enabled, user can connect guest OS running inside hypervisor through
    /// "kata-runtime exec <sandbox-id>" command
    #[serde(default)]
    pub debug_console_enabled: bool,

    /// Agent server port
    #[serde(default = "default_server_port")]
    pub server_port: u32,

    /// Agent log port
    #[serde(default = "default_log_port")]
    pub log_port: u32,

    /// Agent process io port
    #[serde(default = "default_passfd_listener_port")]
    pub passfd_listener_port: u32,

    /// Agent connection dialing timeout value in millisecond
    #[serde(default = "default_dial_timeout")]
    pub dial_timeout_ms: u32,

    /// Agent reconnect timeout value in millisecond
    #[serde(default = "default_reconnect_timeout")]
    pub reconnect_timeout_ms: u32,

    /// Agent request timeout value in millisecond
    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u32,

    /// Agent health check request timeout value in millisecond
    #[serde(default = "default_health_check_timeout")]
    pub health_check_request_timeout_ms: u32,

    /// Comma separated list of kernel modules and their parameters.
    ///
    /// These modules will be loaded in the guest kernel using modprobe(8).
    /// The following example can be used to load two kernel modules with parameters:
    ///  - kernel_modules=["e1000e InterruptThrottleRate=3000,3000,3000 EEE=1", "i915 enable_ppgtt=0"]
    /// The first word is considered as the module name and the rest as its parameters.
    /// Container will not be started when:
    /// - A kernel module is specified and the modprobe command is not installed in the guest
    ///   or it fails loading the module.
    /// - The module is not available in the guest or it doesn't met the guest kernel
    ///    requirements, like architecture and version.
    #[serde(default)]
    pub kernel_modules: Vec<String>,

    /// container pipe size
    #[serde(default)]
    pub container_pipe_size: u32,

    /// Memory agent configuration
    #[serde(default)]
    pub mem_agent: MemAgent,
}

impl std::default::Default for Agent {
    fn default() -> Self {
        Self {
            debug: true,
            log_level: "info".to_string(),
            enable_tracing: false,
            debug_console_enabled: false,
            server_port: DEFAULT_AGENT_VSOCK_PORT,
            log_port: DEFAULT_AGENT_LOG_PORT,
            passfd_listener_port: DEFAULT_PASSFD_LISTENER_PORT,
            dial_timeout_ms: DEFAULT_AGENT_DIAL_TIMEOUT_MS,
            reconnect_timeout_ms: 3_000,
            request_timeout_ms: 30_000,
            health_check_request_timeout_ms: 90_000,
            kernel_modules: Default::default(),
            container_pipe_size: 0,
            mem_agent: MemAgent::default(),
        }
    }
}

fn default_agent_log_level() -> String {
    String::from("info")
}

fn default_server_port() -> u32 {
    DEFAULT_AGENT_VSOCK_PORT
}

fn default_log_port() -> u32 {
    DEFAULT_AGENT_LOG_PORT
}

fn default_passfd_listener_port() -> u32 {
    DEFAULT_PASSFD_LISTENER_PORT
}

fn default_dial_timeout() -> u32 {
    // ms
    10
}

fn default_reconnect_timeout() -> u32 {
    // ms
    3_000
}

fn default_request_timeout() -> u32 {
    // ms
    30_000
}

fn default_health_check_timeout() -> u32 {
    // ms
    90_000
}

impl Agent {
    fn validate(&self) -> Result<()> {
        if self.dial_timeout_ms == 0 {
            return Err(eother!("dial_timeout_ms couldn't be 0."));
        }

        Ok(())
    }
}

impl ConfigOps for Agent {
    fn adjust_config(conf: &mut TomlConfig) -> Result<()> {
        AgentVendor::adjust_config(conf)?;
        Ok(())
    }

    fn validate(conf: &TomlConfig) -> Result<()> {
        AgentVendor::validate(conf)?;
        for (_, agent_config) in conf.agent.iter() {
            agent_config.validate()?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "enable-vendor"))]
mod vendor {
    use super::*;

    /// Vendor customization agent configuration.
    #[derive(Debug, Default, Deserialize, Serialize)]
    pub struct AgentVendor {}

    impl ConfigOps for AgentVendor {}
}

#[cfg(feature = "enable-vendor")]
#[path = "agent_vendor.rs"]
mod vendor;
