// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

//! Operational configuration loading shared by runtime entry points and tools.
//!
//! Configuration schemas, parsing, and hypervisor-specific adjustments remain
//! owned by `kata-types`; this module owns their runtime ordering and inputs.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use hypervisor::Param;
use kata_types::{annotations::Annotation, config::TomlConfig};
use protobuf::Message as ProtobufMessage;
use slog::info;
use tracing::instrument;

const KATA_CONF_FILE: &str = "KATA_CONF_FILE";

/// Configuration selected by shim bootstrap and inherited by its child.
pub const SHIM_CONFIG_PATH_ENV: &str = "KATA_SHIM_CONFIG_PATH";

/// Select a path without loading or adjusting configuration (also used before logging).
pub fn runtime_config_path(runtime_options: Option<&[u8]>) -> Result<String> {
    select_config_path(
        std::env::var(KATA_CONF_FILE).ok(),
        runtime_options,
        std::env::var(SHIM_CONFIG_PATH_ENV).unwrap_or_default(),
    )
}

/// Loads and finalizes runtime configuration for operational use.
///
/// Config override ordering(high to low):
/// 1. environment variable
/// 2. shimv2 create task option
/// 3. shim bootstrap selection
/// 4. If above values are empty, get a default path from DEFAULT_RUNTIME_CONFIGURATIONS
/// in kata-containers/src/libs/kata-types/src/config/default.rs, in array order.
#[instrument]
pub fn load_config(an: &HashMap<String, String>, option: &Option<Vec<u8>>) -> Result<TomlConfig> {
    Ok(load_config_with_path(an, option)?.0)
}

/// Like `load_config`, also returning the configuration file that was loaded.
pub fn load_config_with_path(
    an: &HashMap<String, String>,
    option: &Option<Vec<u8>>,
) -> Result<(TomlConfig, PathBuf)> {
    let config_path = requested_config_path(option)?;
    load_config_from(an, &config_path)
}

/// Configuration file requested by the environment or the shimv2 options. An
/// empty path selects the first existing default configuration file.
fn requested_config_path(option: &Option<Vec<u8>>) -> Result<String> {
    runtime_config_path(option.as_deref())
}

/// Hypervisor configuration plugins compiled into the caller must be registered
/// before invoking this function.
pub fn load_runtime_config(
    annotations: &HashMap<String, String>,
    runtime_options: Option<&[u8]>,
) -> Result<(TomlConfig, PathBuf)> {
    let config_path = runtime_config_path(runtime_options)?;
    load_config_file(annotations, &config_path)
}

fn select_config_path(
    kata_conf_file: Option<String>,
    option: Option<&[u8]>,
    bootstrap_path: String,
) -> Result<String> {
    let logger = slog::Logger::clone(&slog_scope::logger());

    if let Some(path) = kata_conf_file {
        if is_shipped_kata_config_path(&path) {
            Ok(path)
        } else {
            Err(anyhow!(
                "invalid KATA_CONF_FILE {:?}: only shipped Kata configuration files are accepted",
                path
            ))
        }
    } else if let Some(option) = option {
        match <protocols::runtimeoptions::Options as ProtobufMessage>::parse_from_bytes(option) {
            Ok(options) if !options.config_path.is_empty() => Ok(options.config_path),
            Ok(_) => Ok(bootstrap_path),
            Err(error) => {
                slog::warn!(
                    logger,
                    "failed to parse containerd runtime options: {}, falling back to bootstrap/default config",
                    error
                );

                Ok(bootstrap_path)
            }
        }
    } else {
        Ok(bootstrap_path)
    }
}

fn is_shipped_kata_config_path(config_path: &str) -> bool {
    config_path_matches_defaults(config_path, TomlConfig::get_default_config_file_list())
}

fn config_path_matches_defaults(config_path: &str, default_config_paths: Vec<PathBuf>) -> bool {
    let Ok(resolved_config_path) = std::fs::canonicalize(config_path) else {
        return false;
    };

    default_config_paths
        .into_iter()
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .any(|path| path == resolved_config_path)
}

/// Load `config_path`, or the first existing default configuration file when
/// it is empty, returning the configuration and the file that was loaded.
pub fn load_config_from<P: AsRef<Path> + std::fmt::Debug>(
    an: &HashMap<String, String>,
    config_path: P,
) -> Result<(TomlConfig, PathBuf)> {
    let annotation = Annotation::new(an.clone());
    // Clone a logger from global logger to ensure the logs in this function get flushed when drop
    let logger = slog::Logger::clone(&slog_scope::logger());

    info!(logger, "get config path {:?}", &config_path);
    let (mut toml_config, loaded_path) =
        TomlConfig::load_from_file(&config_path).context(format!(
            "load TOML config failed (tried {:?})",
            TomlConfig::get_default_config_file_list()
        ))?;
    annotation.update_config_by_annotation(&mut toml_config)?;
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    info!(logger, "get config content {:?}", &toml_config);
    Ok((toml_config, loaded_path))
}

fn load_config_file(
    annotations: &HashMap<String, String>,
    config_path: &str,
) -> Result<(TomlConfig, PathBuf)> {
    load_config_from(annotations, config_path)
}

// The agent reads these parameters from the guest command line.
fn update_agent_kernel_params(config: &mut TomlConfig) -> Result<()> {
    let mut params = Vec::new();
    if let Ok(agent_params) = config.get_agent_kernel_params() {
        for (key, value) in agent_params {
            if let Ok(param) = Param::new(&key, &value).to_string() {
                params.push(param);
            }
        }
        if let Some(hypervisor) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
            hypervisor.boot_info.add_kernel_params(params);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::{
        annotations::KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY,
        config::{default, QemuConfig},
    };

    #[test]
    fn config_path_matches_only_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let shipped_path = temp_dir.path().join("shipped.toml");
        let non_shipped_path = temp_dir.path().join("non-shipped.toml");
        std::fs::write(&shipped_path, b"[hypervisor.qemu]\n").unwrap();
        std::fs::write(&non_shipped_path, b"[hypervisor.qemu]\n").unwrap();

        let default_paths = vec![shipped_path.clone()];
        assert!(config_path_matches_defaults(
            &shipped_path.to_string_lossy(),
            default_paths.clone()
        ));
        assert!(!config_path_matches_defaults(
            &non_shipped_path.to_string_lossy(),
            default_paths.clone()
        ));
        assert!(!config_path_matches_defaults(
            &temp_dir.path().join("missing.toml").to_string_lossy(),
            default_paths.clone()
        ));
        assert!(!config_path_matches_defaults("", default_paths));
    }

    #[test]
    fn config_source_uses_runtime_options_unless_env_is_set() {
        let mut options = protocols::runtimeoptions::Options::new();
        options.config_path = "/tmp/runtime-options.toml".to_string();
        let bytes = options.write_to_bytes().unwrap();

        assert_eq!(
            select_config_path(None, Some(&bytes), "/bootstrap.toml".into()).unwrap(),
            options.config_path
        );
        assert!(select_config_path(
            Some("/tmp/non-shipped.toml".to_string()),
            Some(&bytes),
            String::new()
        )
        .is_err());
    }

    #[test]
    fn empty_or_invalid_options_preserve_bootstrap_selection() {
        for options in [None, Some(&[][..]), Some(&[0xff][..])] {
            assert_eq!(
                select_config_path(None, options, "/bootstrap.toml".into()).unwrap(),
                "/bootstrap.toml"
            );
            assert_eq!(
                select_config_path(None, options, String::new()).unwrap(),
                ""
            );
        }
    }

    #[test]
    fn canonical_loader_applies_adjustments_annotations_and_agent_params() {
        QemuConfig::new().register();
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("configuration.toml");
        std::fs::write(
            &config_path,
            r#"
[hypervisor.qemu]
path = "/bin/echo"
ctlpath = "/bin/echo"
kernel = "/bin/echo"
image = "/bin/echo"
firmware = ""
default_bridges = 0
enable_annotations = ["default_memory"]

[agent.kata]
container_pipe_size = 17

[runtime]
name = "virt_container"
hypervisor_name = "qemu"
agent_name = "kata"
"#,
        )
        .unwrap();

        let annotations = HashMap::from([(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY.to_string(),
            "768".to_string(),
        )]);
        let (config, loaded_path) =
            load_config_file(&annotations, &config_path.to_string_lossy()).unwrap();
        let hypervisor = &config.hypervisor["qemu"];

        assert_eq!(loaded_path, config_path.canonicalize().unwrap());
        assert_eq!(
            hypervisor.device_info.default_bridges,
            default::DEFAULT_QEMU_PCI_BRIDGES
        );
        assert_eq!(hypervisor.memory_info.default_memory, 768);
        assert!(hypervisor
            .boot_info
            .kernel_params
            .contains("agent.container_pipe_size=17"));
    }

    #[test]
    fn canonical_loader_rejects_invalid_final_config() {
        QemuConfig::new().register();
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("invalid-configuration.toml");
        std::fs::write(
            &config_path,
            r#"
[hypervisor.qemu]
path = "/bin/echo"
ctlpath = "/bin/echo"
kernel = "/bin/echo"
image = "/bin/echo"
firmware = ""

[agent.kata]
dial_timeout_ms = 0

[runtime]
name = "virt_container"
hypervisor_name = "qemu"
agent_name = "kata"
"#,
        )
        .unwrap();

        let error = load_config_file(&HashMap::new(), &config_path.to_string_lossy()).unwrap_err();

        assert_eq!(
            error.root_cause().to_string(),
            "dial_timeout_ms couldn't be 0."
        );
    }
}
