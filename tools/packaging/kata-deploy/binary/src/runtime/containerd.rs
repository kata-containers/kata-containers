// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::k8s;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::{Context, Result};
use log::info;
use std::fs;
use std::path::Path;

pub async fn configure_containerd_runtime(
    config: &Config,
    runtime: &str,
    shim: &str,
) -> Result<()> {
    log::info!("configure_containerd_runtime: Starting for shim={}", shim);
    let adjusted_shim = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{shim}-{suffix}"),
        _ => shim.to_string(),
    };
    let runtime_name = format!("kata-{adjusted_shim}");
    let configuration = format!("configuration-{shim}");

    log::info!("configure_containerd_runtime: Getting containerd paths");
    let paths = config.get_containerd_paths(runtime).await?;
    log::info!("configure_containerd_runtime: use_drop_in={}", paths.use_drop_in);

    let configuration_file: std::path::PathBuf = if paths.use_drop_in {
        // Only add /host prefix if path is not in /etc/containerd (which is mounted from host)
        let base_path = if paths.drop_in_file.starts_with("/etc/containerd/") {
            Path::new(&paths.drop_in_file).to_path_buf()
        } else {
            // Need to add /host prefix for paths outside /etc/containerd
            let drop_in_path = paths.drop_in_file.trim_start_matches('/');
            Path::new("/host").join(drop_in_path)
        };

        log::debug!("Using drop-in config file: {:?}", base_path);
        base_path
    } else {
        log::debug!("Using main config file: {}", paths.config_file);
        Path::new(&paths.config_file).to_path_buf()
    };

    // Use config_file to read containerd version from
    let containerd_root_conf_file = &paths.config_file;

    let pluginid = if fs::read_to_string(containerd_root_conf_file)
        .unwrap_or_default()
        .contains("version = 3")
    {
        "\"io.containerd.cri.v1.runtime\""
    } else if fs::read_to_string(containerd_root_conf_file)
        .unwrap_or_default()
        .contains("version = 2")
    {
        "\"io.containerd.grpc.v1.cri\""
    } else {
        "cri"
    };

    let runtime_table = format!(".plugins.{pluginid}.containerd.runtimes.{runtime_name}");
    let runtime_options_table = format!("{runtime_table}.options");
    let runtime_type = format!("\"io.containerd.{runtime_name}.v2\"");
    let runtime_config_path = format!(
        "\"{}/{}.toml\"",
        utils::get_kata_containers_config_path(shim, &config.dest_dir),
        configuration
    );
    let runtime_path = format!(
        "\"{}\"",
        utils::get_kata_containers_runtime_path(shim, &config.dest_dir)
    );

    log::info!(
        "configure_containerd_runtime: Writing to config file: {:?}",
        configuration_file
    );
    log::info!("configure_containerd_runtime: Setting runtime_type");
    toml_utils::set_toml_value(
        &configuration_file,
        &format!("{runtime_table}.runtime_type"),
        &runtime_type,
    )?;
    toml_utils::set_toml_value(
        &configuration_file,
        &format!("{runtime_table}.runtime_path"),
        &runtime_path,
    )?;
    toml_utils::set_toml_value(
        &configuration_file,
        &format!("{runtime_table}.privileged_without_host_devices"),
        "true",
    )?;

    let pod_annotations = if shim.contains("nvidia-gpu-") {
        "[\"io.katacontainers.*\",\"cdi.k8s.io/*\"]"
    } else {
        "[\"io.katacontainers.*\"]"
    };
    toml_utils::set_toml_value(
        &configuration_file,
        &format!("{runtime_table}.pod_annotations"),
        pod_annotations,
    )?;

    toml_utils::set_toml_value(
        &configuration_file,
        &format!("{runtime_options_table}.ConfigPath"),
        &runtime_config_path,
    )?;

    if config.debug {
        toml_utils::set_toml_value(&configuration_file, ".debug.level", "\"debug\"")?;
    }

    match config.snapshotter_handler_mapping_for_arch.as_ref() {
        Some(mapping) => {
            let snapshotters: Vec<&str> = mapping.split(',').collect();
            for m in snapshotters {
                // Format is already validated in snapshotter_handler_mapping_validation_check
                // and should be validated in Helm templates
                let parts: Vec<&str> = m.split(':').collect();
                let key = parts[0];
                let value = parts[1];

                if key != shim {
                    continue;
                }

                let snapshotter_value = if value == "nydus" {
                    match config.multi_install_suffix.as_ref() {
                        Some(suffix) if !suffix.is_empty() => format!("\"{value}-{suffix}\""),
                        _ => format!("\"{value}\""),
                    }
                } else {
                    format!("\"{value}\"")
                };

                toml_utils::set_toml_value(
                    &configuration_file,
                    &format!("{runtime_table}.snapshotter"),
                    &snapshotter_value,
                )?;
                break;
            }
        }
        _ => {}
    }

    Ok(())
}

pub async fn configure_containerd(config: &Config, runtime: &str) -> Result<()> {
    info!("Add Kata Containers as a supported runtime for containerd");

    fs::create_dir_all("/etc/containerd/")?;

    // Get all paths and drop-in capability in one call
    let paths = config.get_containerd_paths(runtime).await?;

    if !paths.use_drop_in {
        // For non-drop-in, backup the correct config file for each runtime
        if Path::new(&paths.config_file).exists() && !Path::new(&paths.backup_file).exists() {
            fs::copy(&paths.config_file, &paths.backup_file)?;
        }
    } else {
        // Create the drop-in file directory and file
        // Only add /host prefix if path is not in /etc/containerd (which is mounted from host)
        let drop_in_file = if paths.drop_in_file.starts_with("/etc/containerd/") {
            paths.drop_in_file.clone()
        } else {
            format!("/host{}", paths.drop_in_file)
        };
        log::info!("Creating drop-in file at: {}", drop_in_file);

        if let Some(parent) = Path::new(&drop_in_file).parent() {
            log::info!("Creating parent directory: {:?}", parent);
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {parent:?}"))?;
            log::info!("Successfully created parent directory");
        }

        // Create the file if it doesn't exist (with empty content)
        if !Path::new(&drop_in_file).exists() {
            log::info!("Drop-in file doesn't exist, creating it");
            fs::write(&drop_in_file, "")
                .with_context(|| format!("Failed to create drop-in file: {drop_in_file}"))?;
            log::info!("Successfully created drop-in file");
        } else {
            log::info!("Drop-in file already exists");
        }

        // Add the drop-in file to the imports array in the main config
        if let Some(imports_file) = &paths.imports_file {
            log::info!("Adding drop-in to imports in: {}", imports_file);
            let imports_path = ".imports";
            let drop_in_path = format!("\"{}\"", paths.drop_in_file);

            toml_utils::append_to_toml_array(
                Path::new(imports_file),
                imports_path,
                &drop_in_path,
            )?;
            log::info!("Successfully added drop-in to imports array");
        } else {
            log::info!("Runtime auto-loads drop-in files, skipping imports");
        }
    }

    log::info!("Configuring {} shim(s)", config.shims_for_arch.len());
    for shim in &config.shims_for_arch {
        log::info!("Configuring runtime for shim: {}", shim);
        configure_containerd_runtime(config, runtime, shim).await?;
        log::info!("Successfully configured runtime for shim: {}", shim);
    }

    log::info!("Successfully configured all containerd runtimes");
    Ok(())
}

pub async fn cleanup_containerd(config: &Config, runtime: &str) -> Result<()> {
    // Get all paths and drop-in capability in one call
    let paths = config.get_containerd_paths(runtime).await?;

    if paths.use_drop_in {
        // Remove drop-in from imports array (if imports are used)
        if let Some(imports_file) = &paths.imports_file {
            toml_utils::remove_from_toml_array(
                Path::new(imports_file),
                ".imports",
                &format!("\"{}\"", paths.drop_in_file),
            )?;
        }
        return Ok(());
    }

    // For non-drop-in, restore from backup
    if Path::new(&paths.backup_file).exists() {
        fs::remove_file(&paths.config_file)?;
        fs::rename(&paths.backup_file, &paths.config_file)?;
    } else {
        fs::remove_file(&paths.config_file).ok();
    }

    Ok(())
}

/// Setup containerd config files based on runtime type
pub fn setup_containerd_config_files(runtime: &str, config: &Config) -> Result<()> {
    match runtime {
        "k3s" | "k3s-agent" | "rke2-agent" | "rke2-server" => {
            let tmpl_file = format!("{}.tmpl", config.containerd_conf_file);
            if !Path::new(&tmpl_file).exists() && Path::new(&config.containerd_conf_file).exists() {
                fs::copy(&config.containerd_conf_file, &tmpl_file)?;
            }
        }
        "k0s-worker" | "k0s-controller" => {
            // k0s uses /etc/containerd/containerd.d/ for drop-ins (no /host prefix needed)
            // Path is fixed for k0s, so we can hardcode it here
            let drop_in_file_path = "/etc/containerd/containerd.d/kata-deploy.toml";
            if let Some(parent) = Path::new(drop_in_file_path).parent() {
                fs::create_dir_all(parent)?;
            }
            fs::File::create(drop_in_file_path)?;
        }
        "containerd" => {
            if !Path::new(&config.containerd_conf_file).exists() {
                if let Some(parent) = Path::new(&config.containerd_conf_file).parent() {
                    if parent.exists() {
                        // Write output to file
                        let output = utils::host_exec(&["containerd", "config", "default"])?;
                        fs::write(&config.containerd_conf_file, output)?;
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// Check if containerd version supports snapshotter configuration
/// Returns Ok(()) if version is supported, Err if version is too old
fn check_containerd_snapshotter_version_support(
    container_runtime_version: &str,
    has_snapshotter_mapping: bool,
) -> Result<()> {
    let containerd_prefix = "containerd://";
    let containerd_version_to_avoid = "1.6";
    let containerd_version = container_runtime_version
        .strip_prefix(containerd_prefix)
        .unwrap_or(container_runtime_version);

    if containerd_version.starts_with(containerd_version_to_avoid) && has_snapshotter_mapping {
        return Err(anyhow::anyhow!(
            "kata-deploy only supports snapshotter configuration with containerd 1.7 or newer"
        ));
    }

    Ok(())
}

pub async fn containerd_snapshotter_version_check(config: &Config) -> Result<()> {
    let container_runtime_version =
        k8s::get_node_field(config, ".status.nodeInfo.containerRuntimeVersion").await?;

    let has_snapshotter_mapping = config
        .snapshotter_handler_mapping_for_arch
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    check_containerd_snapshotter_version_support(&container_runtime_version, has_snapshotter_mapping)
}

fn check_containerd_erofs_version_support(container_runtime_version: &str) -> Result<()> {
    let containerd_prefix = "containerd://";
    let containerd_version = container_runtime_version
        .strip_prefix(containerd_prefix)
        .unwrap_or(container_runtime_version);

    let min_version_major = 2;
    let min_version_minor = 2;

    let parts: Vec<&str> = containerd_version.split('.').collect();
    if parts.len() < 2 {
        return Err(anyhow::anyhow!("Invalid containerd version format"));
    }

    let major: u32 = parts[0].parse().context("Failed to parse major version")?;
    let minor_str: String = parts[1]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let minor: u32 = minor_str.parse().context("Failed to parse minor version")?;

    if min_version_major > major || (min_version_major == major && min_version_minor > minor) {
        return Err(anyhow::anyhow!(
            "In order to use erofs-snapshotter containerd must be 2.2.0 or newer"
        ));
    }

    Ok(())
}

pub async fn containerd_erofs_snapshotter_version_check(config: &Config) -> Result<()> {
    let container_runtime_version =
        k8s::get_node_field(config, ".status.nodeInfo.containerRuntimeVersion").await?;

    check_containerd_erofs_version_support(&container_runtime_version)
}

pub fn snapshotter_handler_mapping_validation_check(config: &Config) -> Result<()> {
    info!(
        "Validating the snapshotter-handler mapping: \"{:?}\"",
        config.snapshotter_handler_mapping_for_arch
    );

    let mapping = match config.snapshotter_handler_mapping_for_arch.as_ref() {
        Some(m) => m,
        None => {
            info!("No snapshotter has been requested, using the default value from containerd");
            return Ok(());
        }
    };

    let snapshotters: Vec<&str> = mapping.split(',').collect();
    for m in &snapshotters {
        let parts: Vec<&str> = m.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format"
            ));
        }

        let shim = parts[0];
        let snapshotter = parts[1];

        if shim.is_empty() {
            return Err(anyhow::anyhow!(
                "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format, but at least one shim is empty"
            ));
        }

        if snapshotter.is_empty() {
            return Err(anyhow::anyhow!(
                "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format, but at least one snapshotter is empty"
            ));
        }

        if !config.shims_for_arch.contains(&shim.to_string()) {
            return Err(anyhow::anyhow!(
                "\"{}\" is not part of \"{}\"",
                shim,
                config.shims_for_arch.join(" ")
            ));
        }

        let matches: Vec<&&str> = snapshotters
            .iter()
            .filter(|s| s.starts_with(&format!("{shim}:")))
            .collect();
        if matches.len() != 1 {
            return Err(anyhow::anyhow!(
                "One, and only one, entry per shim is required"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_containerd_snapshotter_version_support_1_6_with_mapping() {
        // Version 1.6 with snapshotter mapping should fail
        let result = check_containerd_snapshotter_version_support("containerd://1.6.28", true);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("kata-deploy only supports snapshotter configuration with containerd 1.7 or newer"));
    }

    #[test]
    fn test_check_containerd_snapshotter_version_support_1_6_without_mapping() {
        // Version 1.6 without snapshotter mapping should pass (no mapping means no check needed)
        let result = check_containerd_snapshotter_version_support("containerd://1.6.28", false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_containerd_snapshotter_version_support_1_7_with_mapping() {
        // Version 1.7 with snapshotter mapping should pass
        let result = check_containerd_snapshotter_version_support("containerd://1.7.15", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_containerd_snapshotter_version_support_2_0_with_mapping() {
        // Version 2.0 with snapshotter mapping should pass
        let result = check_containerd_snapshotter_version_support("containerd://2.0.0", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_containerd_snapshotter_version_support_without_prefix() {
        // Version without containerd:// prefix should still work
        let result = check_containerd_snapshotter_version_support("1.6.28", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_containerd_snapshotter_version_support_1_6_variants() {
        // Test various 1.6.x versions
        assert!(check_containerd_snapshotter_version_support("containerd://1.6.0", true).is_err());
        assert!(check_containerd_snapshotter_version_support("containerd://1.6.28", true).is_err());
        assert!(check_containerd_snapshotter_version_support("containerd://1.6.999", true).is_err());
    }

    #[test]
    fn test_check_containerd_snapshotter_version_support_1_7_variants() {
        // Test various 1.7+ versions should pass
        assert!(check_containerd_snapshotter_version_support("containerd://1.7.0", true).is_ok());
        assert!(check_containerd_snapshotter_version_support("containerd://1.7.15", true).is_ok());
        assert!(check_containerd_snapshotter_version_support("containerd://1.8.0", true).is_ok());
    }

    #[test]
    fn test_check_containerd_erofs_version_support() {
        // Versions that should pass (2.2.0+)
        let passing_versions = [
            "containerd://2.2.0",
            "containerd://2.2.0-rc.1",
            "containerd://2.2.1",
            "containerd://2.3.0",
            "containerd://3.0.0",
            "containerd://2.3.0-beta.0",
            "2.2.0", // without prefix
        ];
        for version in passing_versions {
            assert!(
                check_containerd_erofs_version_support(version).is_ok(),
                "Expected {} to pass",
                version
            );
        }

        // Versions that should fail (< 2.2.0)
        let failing_versions = [
            ("containerd://2.1.0", "containerd must be 2.2.0 or newer"),
            ("containerd://2.1.5-rc.1", "containerd must be 2.2.0 or newer"),
            ("containerd://2.0.0", "containerd must be 2.2.0 or newer"),
            ("containerd://1.7.0", "containerd must be 2.2.0 or newer"),
            ("containerd://1.6.28", "containerd must be 2.2.0 or newer"),
            ("2.1.0", "containerd must be 2.2.0 or newer"), // without prefix
            ("invalid", "Invalid containerd version format"),
            ("containerd://abc.2.0", "Failed to parse major version"),
        ];
        for (version, expected_error) in failing_versions {
            let result = check_containerd_erofs_version_support(version);
            assert!(result.is_err(), "Expected {} to fail", version);
            assert!(
                result.unwrap_err().to_string().contains(expected_error),
                "Expected error for {} to contain '{}'",
                version,
                expected_error
            );
        }
    }
}
