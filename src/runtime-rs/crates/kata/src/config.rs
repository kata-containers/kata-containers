// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};
use virtcontainers::config::TomlConfig;

const ANNOTATION_SANDBOX_CONFIG_PATH_KEY: &str = "io.katacontainers.config_path";

const DEFAULT_CONFIG_PATHS: &[&str] = &[
    "/etc/kata-containers/configuration.toml",
    "/usr/share/defaults/kata-containers/configuration.toml",
];

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to find configuration")]
    Get,
    #[error("failed to load configuration: {0}")]
    Load(#[source] std::io::Error),
    #[error("failed to parse configuration: {0}")]
    Parse(#[source] toml::de::Error),
}

fn get_config_path_from_oci_spec(bundle_path: &Path) -> Option<String> {
    let spec_file = bundle_path.join("config.json");
    let spec = &oci_spec::runtime::Spec::load(spec_file).ok()?;

    spec.annotations()
        .as_ref()?
        .get(ANNOTATION_SANDBOX_CONFIG_PATH_KEY)
        .map(|v| v.into())
}

fn get_config_path_from_env() -> Option<String> {
    std::env::var("KATA_CONF_FILE").ok()
}

fn get_preferred_config_path(bundle_path: &Path) -> Option<PathBuf> {
    let path = get_config_path_from_oci_spec(bundle_path).or_else(get_config_path_from_env)?;
    std::fs::canonicalize(path).ok()
}

fn get_default_config_path() -> Option<PathBuf> {
    for f in DEFAULT_CONFIG_PATHS {
        if let Ok(path) = std::fs::canonicalize(f) {
            return Some(path);
        }
    }
    None
}

/// Try to load configuration information from a configuration file in following order:
/// - configuration file specified by config.json from OCI image bundle
/// - configuration file specified by the "KATA_CONF_FILE" environment variable
/// - /etc/kata-containers/configuration.toml
/// - /usr/share/defaults/kata-containers/configuration.toml
pub fn load_configuration(bundle_path: &Path) -> Result<TomlConfig, ConfigError> {
    let path = get_preferred_config_path(bundle_path)
        .or_else(get_default_config_path)
        .ok_or(ConfigError::Get)?;
    let content = std::fs::read_to_string(path).map_err(ConfigError::Load)?;

    toml::from_str(&content).map_err(ConfigError::Parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_from_env() {
        let file = tests_utils::get_kata_config_file();
        std::env::set_var("KATA_CONF_FILE", file);
        let bundle_path = tests_utils::get_image_bundle_path();
        let config = load_configuration(&bundle_path).unwrap();
        assert_eq!(config.runtime.debug, true);

        let dragonball = config.hypervisor.get("dragonball").unwrap();
        assert_eq!(dragonball.default_vcpus, 2);

        let qemu = config.hypervisor.get("qemu").unwrap();
        assert_eq!(qemu.default_vcpus, 4);
    }
}
