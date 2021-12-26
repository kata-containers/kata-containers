// Copyright (c) 2019-2021 Ant Financial
// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs;
use std::io::{self, Result};
use std::path::{Path, PathBuf};

use crate::{eother, sl};

/// Default configuration values.
pub mod default;

mod hypervisor;
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
        info!(sl!(), "get kata config: {:?}", config);

        Ok(config)
    }

    /// Validate Kata configuration information.
    pub fn validate(&self) -> Result<()> {
        Hypervisor::validate(self)?;
        Runtime::validate(self)?;

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
