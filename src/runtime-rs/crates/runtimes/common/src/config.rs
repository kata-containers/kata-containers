// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use hypervisor::Param;
use kata_types::{annotations::Annotation, config::TomlConfig};
use protobuf::Message as ProtobufMessage;
use slog::info;
use tracing::instrument;

/// Hypervisor plugins must be registered first, or hypervisor defaults are not applied.
///
/// Config override ordering(high to low):
/// 1. environment variable
/// 2. shimv2 create task option
/// 3. If above two are not set, then get default path from DEFAULT_RUNTIME_CONFIGURATIONS
/// in kata-containers/src/libs/kata-types/src/config/default.rs, in array order.
#[instrument]
pub fn load_config(an: &HashMap<String, String>, option: &Option<Vec<u8>>) -> Result<TomlConfig> {
    const KATA_CONF_FILE: &str = "KATA_CONF_FILE";
    let annotation = Annotation::new(an.clone());
    // Clone a logger from global logger to ensure the logs in this function get flushed when drop
    let logger = slog::Logger::clone(&slog_scope::logger());

    let config_path = if let Ok(path) = std::env::var(KATA_CONF_FILE) {
        if is_shipped_kata_config_path(&path) {
            path
        } else {
            return Err(anyhow!(
                "invalid KATA_CONF_FILE {:?}: only shipped Kata configuration files are accepted",
                path
            ));
        }
    } else if let Some(option) = option {
        // Parse the containerd runtime options protobuf message to extract the config path.
        // The options are passed as a serialized runtimeoptions.v1.Options protobuf message
        // from containerd's configuration (e.g., [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata.options]).
        match <protocols::runtimeoptions::Options as ProtobufMessage>::parse_from_bytes(option) {
            Ok(opts) => opts.config_path,
            Err(e) => {
                // Log the error but don't fail - fall back to default config paths
                let logger = slog::Logger::clone(&slog_scope::logger());
                slog::warn!(
                    logger,
                    "failed to parse containerd runtime options: {}, falling back to default config paths",
                    e
                );
                String::from("")
            }
        }
    } else {
        String::from("")
    };

    info!(logger, "get config path {:?}", &config_path);
    let (mut toml_config, _) = TomlConfig::load_from_file(&config_path).context(format!(
        "load TOML config failed (tried {:?})",
        TomlConfig::get_default_config_file_list()
    ))?;
    annotation.update_config_by_annotation(&mut toml_config)?;
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    info!(logger, "get config content {:?}", &toml_config);
    Ok(toml_config)
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

// this update the agent-specfic kernel parameters into hypervisor's bootinfo
// the agent inside the VM will read from file cmdline to get the params and function
fn update_agent_kernel_params(config: &mut TomlConfig) -> Result<()> {
    let mut params = vec![];
    if let Ok(kv) = config.get_agent_kernel_params() {
        for (k, v) in kv.into_iter() {
            if let Ok(s) = Param::new(k.as_str(), v.as_str()).to_string() {
                params.push(s);
            }
        }
        if let Some(h) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
            h.boot_info.add_kernel_params(params);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[derive(Debug)]
    enum ConfigPathCase {
        Shipped,
        NonShipped,
        NonExistent,
        Empty,
    }

    #[rstest]
    #[case::shipped_config_is_accepted(ConfigPathCase::Shipped, true)]
    #[case::non_shipped_config_is_rejected(ConfigPathCase::NonShipped, false)]
    #[case::non_existent_path_is_rejected(ConfigPathCase::NonExistent, false)]
    #[case::empty_path_is_rejected(ConfigPathCase::Empty, false)]
    fn test_config_path_matches_defaults(
        #[case] path_case: ConfigPathCase,
        #[case] expected: bool,
    ) {
        let tmpdir = tempfile::tempdir().unwrap();
        let shipped_path = tmpdir.path().join("shipped.toml");
        let non_shipped_path = tmpdir.path().join("malicious.toml");
        std::fs::write(&shipped_path, b"[hypervisor.qemu]\n").unwrap();
        std::fs::write(&non_shipped_path, b"[hypervisor.qemu]\n").unwrap();

        // Only the shipped path is treated as a default config location.
        let default_config_paths = vec![shipped_path.clone()];

        let config_path = match path_case {
            ConfigPathCase::Shipped => shipped_path.to_string_lossy().to_string(),
            ConfigPathCase::NonShipped => non_shipped_path.to_string_lossy().to_string(),
            ConfigPathCase::NonExistent => tmpdir
                .path()
                .join("nonexistent.toml")
                .to_string_lossy()
                .to_string(),
            ConfigPathCase::Empty => String::new(),
        };

        assert_eq!(
            config_path_matches_defaults(&config_path, default_config_paths),
            expected,
            "case {:?}: unexpected result for path {:?}",
            path_case,
            config_path,
        );
    }
}
