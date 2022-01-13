// Copyright (c) 2019-2021 Ant Financial
// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs;
use std::io::{self, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::u32;

use lazy_static::__Deref;
use lazy_static::lazy_static;

use crate::annotations::Annotation;
use crate::{eother, sl};

/// Default configuration values.
pub mod default;

mod agent;
pub use self::agent::{Agent, AgentVendor};

pub mod hypervisor;
pub use self::hypervisor::{
    BootInfo, DragonballConfig, Hypervisor, QemuConfig, HYPERVISOR_NAME_DRAGONBALL,
    HYPERVISOR_NAME_QEMU,
};

mod runtime;
pub use self::runtime::{Runtime, RuntimeVendor};

/// Trait to manipulate global Kata configuration information.
pub trait ConfigPlugin: Send + Sync {
    /// Get the plugin name.
    fn name(&self) -> &str;

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_configuration(&self, _conf: &mut TomlConfig) -> Result<()>;

    /// Validate the configuration information.
    fn validate(&self, _conf: &TomlConfig) -> Result<()>;

    /// get min mem

    fn get_min_memory(&self) -> u32;
}

/// Trait to manipulate Kata configuration information.
pub trait ConfigOps {
    /// Adjust the configuration information after loading from configuration file.
    fn adjust_configuration(_conf: &mut TomlConfig) -> Result<()> {
        Ok(())
    }

    /// Validate the configuration information.
    fn validate(_conf: &TomlConfig) -> Result<()> {
        Ok(())
    }
}

/// Trait to manipulate global Kata configuration information.
pub trait ConfigObjectOps {
    /// Adjust the configuration information after loading from configuration file.
    fn adjust_configuration(&mut self) -> Result<()> {
        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Kata configuration information.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct TomlConfig {
    /// Configuration information for agents.
    #[serde(default)]
    pub agent: HashMap<String, Agent>,
    /// Configuration information for hypervisors.
    #[serde(default)]
    pub hypervisor: HashMap<String, Hypervisor>,
    /// Kata runtime configuration information.
    #[serde(default)]
    pub runtime: Runtime,
}

impl TomlConfig {
    /// Load Kata configuration information from configuration files.
    ///
    /// If `config_file` is valid, it will used, otherwise a built-in default path list will be
    /// scanned.
    pub fn load_from_file<P: AsRef<Path>>(config_file: P) -> Result<(TomlConfig, PathBuf)> {
        let file_path = if !config_file.as_ref().as_os_str().is_empty() {
            fs::canonicalize(config_file)?
        } else {
            Self::get_default_config_file()?
        };

        info!(
            sl!(),
            "load configuration from: {}",
            file_path.to_string_lossy()
        );
        let content = fs::read_to_string(&file_path)?;
        let config = Self::load(&content)?;

        Ok((config, file_path))
    }

    /// Load raw Kata configuration information from configuration files.
    ///
    /// If `config_file` is valid, it will used, otherwise a built-in default path list will be
    /// scanned.
    pub fn load_raw_from_file<P: AsRef<Path>>(config_file: P) -> Result<(TomlConfig, PathBuf)> {
        let file_path = if !config_file.as_ref().as_os_str().is_empty() {
            fs::canonicalize(config_file)?
        } else {
            Self::get_default_config_file()?
        };

        info!(
            sl!(),
            "load configuration from: {}",
            file_path.to_string_lossy()
        );
        let content = fs::read_to_string(&file_path)?;
        let config: TomlConfig = toml::from_str(&content)?;

        Ok((config, file_path))
    }

    /// Load Kata configuration information from string.
    pub fn load(content: &str) -> Result<TomlConfig> {
        let mut config: TomlConfig = toml::from_str(content)?;
        Hypervisor::adjust_configuration(&mut config)?;
        Runtime::adjust_configuration(&mut config)?;
        Agent::adjust_configuration(&mut config)?;
        info!(sl!(), "get kata config: {:?}", config);
        Ok(config)
    }

    /// Validate Kata configuration information.
    pub fn validate(&self) -> Result<()> {
        Hypervisor::validate(self)?;
        Runtime::validate(self)?;
        Agent::validate(self)?;

        Ok(())
    }

    ///  Probe configuration file according to the default configuration file list.
    fn get_default_config_file() -> Result<PathBuf> {
        for f in default::DEFAULT_RUNTIME_CONFIGURATIONS.iter() {
            if let Ok(path) = fs::canonicalize(f) {
                return Ok(path);
            }
        }

        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}

/// Validate the `path` matches one of the pattern in `patterns`.
///
/// Each member in `patterns` is a path pattern as described by glob(3)
pub fn validate_path_pattern<P: AsRef<Path>>(patterns: &[String], path: P) -> Result<()> {
    let path = path
        .as_ref()
        .to_str()
        .ok_or_else(|| eother!("Invalid path {}", path.as_ref().to_string_lossy()))?;
    for p in patterns.iter() {
        if let Ok(glob) = glob::Pattern::new(p) {
            if glob.matches(path) {
                return Ok(());
            }
        }
    }

    Err(eother!("Path {} is not permitted", path))
}

/// Kata configuration information.
pub struct KataConfig {
    config: TomlConfig,
    agent: String,
    hypervisor: String,
}

impl KataConfig {
    /// Set the default Kata configuration object.
    ///
    /// The default Kata configuration information is loaded from system configuration file.
    pub fn set_default_config(config: TomlConfig, hypervisor: &str, agent: &str) {
        let kata = KataConfig {
            config,
            agent: agent.to_string(),
            hypervisor: hypervisor.to_string(),
        };
        *KATA_ACTIVE_CONFIG.lock().unwrap() = Arc::new(kata);
    }

    /// Get the default Kata configuration object.
    ///
    /// The default Kata configuration information is loaded from system configuration file.
    pub fn get_default_config() -> Arc<KataConfig> {
        KATA_ACTIVE_CONFIG.lock().unwrap().clone()
    }

    /// Set the active Kata configuration object.
    ///
    /// The active Kata configuration information is default configuration information patched
    /// with tunable configuration information from annotations.
    pub fn set_active_config(config: TomlConfig, hypervisor: &str, agent: &str) {
        let kata = KataConfig {
            config,
            agent: agent.to_string(),
            hypervisor: hypervisor.to_string(),
        };
        *KATA_ACTIVE_CONFIG.lock().unwrap() = Arc::new(kata);
    }

    /// Get the active Kata configuration object.
    ///
    /// The active Kata configuration information is default configuration information patched
    /// with tunable configuration information from annotations.
    pub fn get_active_config() -> Arc<KataConfig> {
        KATA_ACTIVE_CONFIG.lock().unwrap().clone()
    }
    /// Get the config in use
    pub fn get_config(&self) -> &TomlConfig {
        &self.config
    }
    /// Get the agent mut configuration in use
    pub fn get_agent(&self) -> Option<&Agent> {
        if !self.agent.is_empty() {
            self.config.agent.get(&self.agent)
        } else {
            None
        }
    }

    /// Get the hypervisor configuration in use.
    pub fn get_hypervisor(&self) -> Option<&Hypervisor> {
        if !self.hypervisor.is_empty() {
            self.config.hypervisor.get(&self.hypervisor)
        } else {
            None
        }
    }
}

lazy_static! {
    static ref KATA_DEFAULT_CONFIG: Mutex<Arc<KataConfig>> = {
        let config = TomlConfig::load("").unwrap();
        let kata = KataConfig {
            config,
            agent: String::new(),
            hypervisor: String::new(),
        };

        Mutex::new(Arc::new(kata))
    };
    static ref KATA_ACTIVE_CONFIG: Mutex<Arc<KataConfig>> = {
        let config = TomlConfig::load("").unwrap();
        let kata = KataConfig {
            config,
            agent: String::new(),
            hypervisor: String::new(),
        };
        Mutex::new(Arc::new(kata))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_pattern() {
        let patterns = [];
        validate_path_pattern(&patterns, "/bin/ls").unwrap_err();

        let patterns = ["/bin".to_string()];
        validate_path_pattern(&patterns, "/bin/ls").unwrap_err();

        let patterns = ["/bin/*/ls".to_string()];
        validate_path_pattern(&patterns, "/bin/ls").unwrap_err();

        let patterns = ["/bin/*".to_string()];
        validate_path_pattern(&patterns, "/bin/ls").unwrap();

        let patterns = ["/*".to_string()];
        validate_path_pattern(&patterns, "/bin/ls").unwrap();

        let patterns = ["/usr/share".to_string(), "/bin/*".to_string()];
        validate_path_pattern(&patterns, "/bin/ls").unwrap();
    }
}
