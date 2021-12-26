// Copyright (c) 2019-2021 Ant Financial
// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::io::{self, Result};
use std::path::{Path, PathBuf};

use crate::sl;

/// Default configuration values.
pub mod default;

mod runtime;
pub use self::runtime::{Runtime, RuntimeVendor};

/// Trait to manipulate global Kata configuration information.
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

        Runtime::adjust_configuration(&mut config)?;
        info!(sl!(), "get kata config: {:?}", config);

        Ok(config)
    }

    /// Validate Kata configuration information.
    pub fn validate(&self) -> Result<()> {
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
