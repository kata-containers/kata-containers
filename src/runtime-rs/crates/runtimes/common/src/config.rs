// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

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

const GUEST_APPARMOR_PARAM: &str = "agent.enable_guest_apparmor";

/// Hypervisor plugins must be registered first, or hypervisor defaults are not applied.
///
/// Config override ordering(high to low):
/// 1. environment variable
/// 2. shimv2 create task option
/// 3. If above two are not set, then get default path from DEFAULT_RUNTIME_CONFIGURATIONS
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
    const KATA_CONF_FILE: &str = "KATA_CONF_FILE";

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
    Ok(config_path)
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
    reject_guest_apparmor_overrides(&toml_config, an)?;
    annotation.update_config_by_annotation(&mut toml_config)?;
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    info!(logger, "get config content {:?}", &toml_config);
    Ok((toml_config, loaded_path))
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
    if let Some(h) = config.hypervisor.get(&config.runtime.hypervisor_name) {
        reject_guest_apparmor_kernel_param(&h.boot_info.kernel_params)?;
    }
    let mut params = vec![];
    for (k, v) in config.get_agent_kernel_params()? {
        if let Ok(s) = Param::new(k.as_str(), v.as_str()).to_string() {
            params.push(s);
        }
    }
    let enabled = config
        .agent
        .get(&config.runtime.agent_name)
        .is_some_and(|a| a.enable_guest_apparmor);
    params.push(format!("{GUEST_APPARMOR_PARAM}={enabled}"));
    if let Some(h) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
        h.boot_info.add_kernel_params(params);
        validate_guest_apparmor_kernel_params(&h.boot_info.kernel_params, enabled)?;
    }
    Ok(())
}

/// Only the runtime may emit this parameter, exactly once with its configured value.
pub fn validate_guest_apparmor_kernel_params(params: &str, enabled: bool) -> Result<()> {
    let expected = format!("{GUEST_APPARMOR_PARAM}={enabled}");
    let mut found = params
        .split_ascii_whitespace()
        .filter(|param| param.trim_matches('"').split('=').next() == Some(GUEST_APPARMOR_PARAM));
    if found.next() != Some(expected.as_str()) || found.next().is_some() {
        return Err(anyhow!(
            "guest AppArmor requires exactly one canonical {}",
            expected
        ));
    }
    Ok(())
}

fn reject_guest_apparmor_kernel_param(params: &str) -> Result<()> {
    if params
        .split_ascii_whitespace()
        .any(|param| param.trim_matches('"').split('=').next() == Some(GUEST_APPARMOR_PARAM))
    {
        return Err(anyhow!(
            "{} is reserved; use [agent.kata].enable_guest_apparmor",
            GUEST_APPARMOR_PARAM
        ));
    }
    Ok(())
}

fn reject_guest_apparmor_overrides(
    config: &TomlConfig,
    an: &HashMap<String, String>,
) -> Result<()> {
    // Reject attempts even if another annotation would replace these parameters.
    if let Some(h) = config.hypervisor.get(&config.runtime.hypervisor_name) {
        reject_guest_apparmor_kernel_param(&h.boot_info.kernel_params)?;
    }
    if let Some(params) = an.get(kata_types::annotations::KATA_ANNO_CFG_HYPERVISOR_KERNEL_PARAMS) {
        reject_guest_apparmor_kernel_param(params)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn guest_apparmor_kernel_param_is_reserved() {
        for params in [
            "agent.enable_guest_apparmor",
            "agent.enable_guest_apparmor=true",
            "foo=1\tagent.enable_guest_apparmor=false\nbar=2",
            "\"agent.enable_guest_apparmor=true\"",
            "agent.enable_guest_apparmor=\"false\"",
            "agent.enable_guest_apparmor=true agent.enable_guest_apparmor=false",
        ] {
            assert!(
                reject_guest_apparmor_kernel_param(params).is_err(),
                "{}",
                params
            );
        }
        assert!(reject_guest_apparmor_kernel_param(
            "selinux=0 apparmor=1 agent.enable_guest_apparmor_extra=true"
        )
        .is_ok());
    }

    #[test]
    fn guest_apparmor_final_kernel_param_must_be_unique_and_match_config() {
        for enabled in [false, true] {
            for (params, valid) in [
                (String::new(), false),
                ("agent.enable_guest_apparmor".into(), false),
                (format!("agent.enable_guest_apparmor={enabled}"), true),
                (format!("agent.enable_guest_apparmor={}", !enabled), false),
                (format!("agent.enable_guest_apparmor={enabled} agent.enable_guest_apparmor={enabled}"), false),
                ("agent.enable_guest_apparmor=true agent.enable_guest_apparmor=false".into(), false),
                ("agent.enable_guest_apparmor=1".into(), false),
                (format!("\"agent.enable_guest_apparmor={enabled}\""), false),
            ] {
                assert_eq!(validate_guest_apparmor_kernel_params(&params, enabled).is_ok(), valid, "{params}");
            }
        }
    }

    #[test]
    fn guest_apparmor_rejects_config_and_annotation_overrides() {
        use kata_types::annotations::KATA_ANNO_CFG_HYPERVISOR_KERNEL_PARAMS;
        use kata_types::config::Hypervisor;

        for configured in [false, true] {
            for annotated in [false, true] {
                let mut config = TomlConfig::default();
                config.runtime.hypervisor_name = "qemu".into();
                let mut hv = Hypervisor::default();
                if configured {
                    hv.boot_info.kernel_params = "agent.enable_guest_apparmor=false".into();
                }
                config.hypervisor.insert("qemu".into(), hv);
                let mut annotations = HashMap::new();
                annotations.insert(
                    KATA_ANNO_CFG_HYPERVISOR_KERNEL_PARAMS.into(),
                    if annotated {
                        "agent.enable_guest_apparmor=true".into()
                    } else {
                        "console=hvc0".into()
                    },
                );
                assert_eq!(
                    reject_guest_apparmor_overrides(&config, &annotations).is_err(),
                    configured || annotated
                );
            }
        }
    }

    #[test]
    fn guest_apparmor_has_no_annotation_override() {
        use kata_types::config::{Agent, Hypervisor};

        kata_types::config::QemuConfig::new().register();
        for enabled in [false, true] {
            let mut config = TomlConfig::default();
            config.runtime.name = "virt-container".into();
            config.runtime.hypervisor_name = "qemu".into();
            config.runtime.agent_name = "kata".into();
            config.agent.insert(
                "kata".into(),
                Agent {
                    enable_guest_apparmor: enabled,
                    ..Default::default()
                },
            );
            let mut hv = Hypervisor {
                path: "/bin/echo".into(),
                ..Default::default()
            };
            hv.boot_info.kernel = "/bin/echo".into();
            config.hypervisor.insert("qemu".into(), hv);
            let mut annotations = HashMap::new();
            annotations.insert(
                "io.katacontainers.config.agent.enable_guest_apparmor".into(),
                (!enabled).to_string(),
            );
            Annotation::new(annotations)
                .update_config_by_annotation(&mut config)
                .unwrap();
            assert_eq!(config.agent["kata"].enable_guest_apparmor, enabled);
            update_agent_kernel_params(&mut config).unwrap();
            assert!(config.hypervisor["qemu"]
                .boot_info
                .kernel_params
                .contains(&format!("{GUEST_APPARMOR_PARAM}={enabled}")));
        }
    }

    #[test]
    fn guest_apparmor_kernel_param_matches_config_only() {
        use kata_types::config::{hypervisor::GuestExtensionImage, Agent, Hypervisor};

        for enabled in [false, true] {
            for extension in [false, true] {
                let mut config = TomlConfig::default();
                config.runtime.hypervisor_name = "qemu".into();
                config.runtime.agent_name = "kata".into();
                config.agent.insert(
                    "kata".into(),
                    Agent {
                        enable_guest_apparmor: enabled,
                        ..Default::default()
                    },
                );
                let mut hv = Hypervisor::default();
                hv.boot_info.kernel_params = "selinux=0 apparmor=1".into();
                if extension {
                    hv.guest_extension_images.push(GuestExtensionImage {
                        name: "apparmor".into(),
                        path: "/policy.img".into(),
                        ..Default::default()
                    });
                }
                config.hypervisor.insert("qemu".into(), hv);
                update_agent_kernel_params(&mut config).unwrap();
                let params = &config.hypervisor["qemu"].boot_info.kernel_params;
                assert_eq!(params.matches(GUEST_APPARMOR_PARAM).count(), 1);
                assert!(params
                    .split_ascii_whitespace()
                    .any(|p| p == format!("{GUEST_APPARMOR_PARAM}={enabled}")));
                assert!(params.contains("selinux=0 apparmor=1"));
                // Re-appending parameters cannot silently introduce duplicates.
                assert!(update_agent_kernel_params(&mut config).is_err());
            }
        }
    }

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
