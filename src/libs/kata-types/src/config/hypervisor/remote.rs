use std::io::Result;
use std::path::Path;
use std::sync::Arc;

use crate::{
    config::{
        default::{self, MAX_REMOTE_VCPUS, MIN_REMOTE_MEMORY_SIZE_MB},
        ConfigPlugin,
    },
    eother, resolve_path,
};

use super::register_hypervisor_plugin;

/// Hypervisor name for remote, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_REMOTE: &str = "remote";

/// Configuration information for remote.
#[derive(Default, Debug)]
pub struct RemoteConfig {}

impl RemoteConfig {
    /// Create a new instance of `RemoteConfig`
    pub fn new() -> Self {
        RemoteConfig {}
    }

    /// Register the remote plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_REMOTE, plugin);
    }
}

impl ConfigPlugin for RemoteConfig {
    fn name(&self) -> &str {
        HYPERVISOR_NAME_REMOTE
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut crate::config::TomlConfig) -> Result<()> {
        if let Some(remote) = conf.hypervisor.get_mut(HYPERVISOR_NAME_REMOTE) {
            if remote.remote_hypervisor_socket.is_empty() {
                remote.remote_hypervisor_socket =
                    default::DEFAULT_REMOTE_HYPERVISOR_SOCKET.to_string();
            }
            resolve_path!(
                remote.remote_hypervisor_socket,
                "Remote hypervisor socket `{}` is invalid: {}"
            )?;
            if remote.remote_hypervisor_timeout == 0 {
                remote.remote_hypervisor_timeout = default::DEFAULT_REMOTE_HYPERVISOR_TIMEOUT;
            }
            if remote.memory_info.default_memory == 0 {
                remote.memory_info.default_memory = default::MIN_REMOTE_MEMORY_SIZE_MB;
            }
            if remote.memory_info.memory_slots == 0 {
                remote.memory_info.memory_slots = default::DEFAULT_REMOTE_MEMORY_SLOTS
            }
        }

        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &crate::config::TomlConfig) -> Result<()> {
        if let Some(remote) = conf.hypervisor.get(HYPERVISOR_NAME_REMOTE) {
            if remote.remote_hypervisor_socket.is_empty() {
                return Err(eother!("Remote hypervisor sock is not set"));
            }
            if remote.remote_hypervisor_timeout == 0 {
                return Err(eother!("Remote hypervisor timeout is not set"));
            }
        }
        Ok(())
    }

    fn get_min_memory(&self) -> u32 {
        MIN_REMOTE_MEMORY_SIZE_MB
    }

    fn get_max_cpus(&self) -> u32 {
        MAX_REMOTE_VCPUS
    }
}
