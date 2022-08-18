// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! A sample for vendor to customize the runtime implementation.

use super::*;
use slog::Level;
/// Vendor customization runtime configuration.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RuntimeVendor {
    /// Log level
    #[serde(default)]
    pub log_level: u32,

    /// Prefix for log messages
    #[serde(default)]
    pub log_prefix: String,
}

impl ConfigOps for RuntimeVendor {
    fn adjust_config(conf: &mut TomlConfig) -> Result<()> {
        if conf.runtime.vendor.log_level > Level::Debug as u32 {
            conf.runtime.debug = true;
        }

        Ok(())
    }

    /// Validate the configuration information.
    fn validate(conf: &TomlConfig) -> Result<()> {
        if conf.runtime.vendor.log_level > 10 {
            return Err(eother!(
                "log level {} in configuration file is invalid",
                conf.runtime.vendor.log_level
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_vendor_config() {
        let content = r#"
[runtime]
debug = false
log_level = 20
log_prefix = "test"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap_err();

        let content = r#"
[runtime]
debug = false
log_level = "test"
log_prefix = "test"
"#;
        TomlConfig::load(content).unwrap_err();
    }

    #[test]
    fn test_vendor_config() {
        let content = r#"
[runtime]
debug = false
log_level = 10
log_prefix = "test"
log_fmt = "nouse"
"#;
        let config: TomlConfig = TomlConfig::load(content).unwrap();
        config.validate().unwrap();
        assert!(config.runtime.debug);
        assert_eq!(config.runtime.vendor.log_level, 10);
        assert_eq!(&config.runtime.vendor.log_prefix, "test");
    }
}
