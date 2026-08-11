// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

//! Operational configuration loading shared by runtime entry points and tools.
//!
//! Configuration schemas, parsing, and hypervisor-specific adjustments remain
//! owned by `kata-types`; this module owns their runtime ordering and inputs.

use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use hypervisor::Param;
use kata_types::{annotations::Annotation, config::TomlConfig};
use protobuf::Message as ProtobufMessage;

const KATA_CONF_FILE: &str = "KATA_CONF_FILE";

/// Loads and finalizes runtime configuration for operational use.
///
/// Hypervisor configuration plugins compiled into the caller must be registered
/// before invoking this function.
pub fn load_runtime_config(
    annotations: &HashMap<String, String>,
    runtime_options: Option<&[u8]>,
) -> Result<(TomlConfig, PathBuf)> {
    let config_path = select_config_path(std::env::var(KATA_CONF_FILE).ok(), runtime_options)?;
    load_runtime_config_from_path(annotations, &config_path)
}

fn select_config_path(
    kata_conf_file: Option<String>,
    runtime_options: Option<&[u8]>,
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
    } else if let Some(runtime_options) = runtime_options {
        match <protocols::runtimeoptions::Options as ProtobufMessage>::parse_from_bytes(
            runtime_options,
        ) {
            Ok(options) => Ok(options.config_path),
            Err(error) => {
                slog::warn!(
                    logger,
                    "failed to parse containerd runtime options: {}, falling back to default config paths",
                    error
                );
                Ok(String::new())
            }
        }
    } else {
        Ok(String::new())
    }
}

fn load_runtime_config_from_path(
    annotations: &HashMap<String, String>,
    config_path: &str,
) -> Result<(TomlConfig, PathBuf)> {
    let annotation = Annotation::new(annotations.clone());
    let logger = slog::Logger::clone(&slog_scope::logger());

    slog::info!(logger, "get config path {:?}", &config_path);
    let (mut config, config_path) = TomlConfig::load_from_file(config_path).context(format!(
        "load TOML config failed (tried {:?})",
        TomlConfig::get_default_config_file_list()
    ))?;
    annotation.update_config_by_annotation(&mut config)?;
    update_agent_kernel_params(&mut config);
    config.validate()?;

    slog::info!(logger, "get config content {:?}", &config);
    Ok((config, config_path))
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

fn update_agent_kernel_params(config: &mut TomlConfig) {
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
            select_config_path(None, Some(&bytes)).unwrap(),
            options.config_path
        );
        assert!(
            select_config_path(Some("/tmp/non-shipped.toml".to_string()), Some(&bytes)).is_err()
        );
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
            load_runtime_config_from_path(&annotations, &config_path.to_string_lossy()).unwrap();
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

        let error = load_runtime_config_from_path(&HashMap::new(), &config_path.to_string_lossy())
            .unwrap_err();

        assert_eq!(
            error.root_cause().to_string(),
            "dial_timeout_ms couldn't be 0."
        );
    }
}
