// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;

use crate::config::{ConfigOps, TomlConfig};

pub use vendor::AgentVendor;

/// Kata agent configuration information.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Agent {
    /// If enabled, the agent will log additional debug messages to the system log.
    #[serde(default, rename = "enable_debug")]
    pub debug: bool,

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

    /// Agent connection dialing timeout value in seconds
    #[serde(default)]
    pub dial_timeout: u32,

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

    /// contianer pipe size
    pub container_pipe_size: u32,
}

impl ConfigOps for Agent {
    fn adjust_configuration(conf: &mut TomlConfig) -> Result<()> {
        AgentVendor::adjust_configuration(conf)?;
        Ok(())
    }

    fn validate(conf: &TomlConfig) -> Result<()> {
        AgentVendor::validate(conf)?;
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
