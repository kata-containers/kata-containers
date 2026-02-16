// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Config, ContainerdPaths, CustomRuntime};
use crate::k8s;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::{Context, Result};
use log::info;
use std::fs;
use std::path::{Path, PathBuf};

struct ContainerdRuntimeParams {
    /// Runtime name (e.g., "kata-qemu")
    runtime_name: String,
    /// Path to the shim binary
    runtime_path: String,
    /// Path to the kata configuration file
    config_path: String,
    /// Pod annotations to allow
    pod_annotations: &'static str,
    /// Optional snapshotter to configure
    snapshotter: Option<String>,
}

/// Plugin ID for CRI runtime in containerd config v3 (version = 3).
const CONTAINERD_V3_RUNTIME_PLUGIN_ID: &str = "\"io.containerd.cri.v1.runtime\"";
/// Plugin ID for CRI in containerd config v2 (version = 2).
const CONTAINERD_V2_CRI_PLUGIN_ID: &str = "\"io.containerd.grpc.v1.cri\"";
/// Legacy plugin key when config has no version (pre-v2).
const CONTAINERD_LEGACY_CRI_PLUGIN_ID: &str = "cri";
/// Plugin ID for CRI images in containerd config v3 (version = 3).
const CONTAINERD_CRI_IMAGES_PLUGIN_ID: &str = "\"io.containerd.cri.v1.images\"";

fn get_containerd_pluginid(config_file: &str) -> Result<&'static str> {
    let content = fs::read_to_string(config_file)
        .with_context(|| format!("Failed to read containerd config file: {}", config_file))?;

    if content.contains("version = 3") {
        Ok(CONTAINERD_V3_RUNTIME_PLUGIN_ID)
    } else if content.contains("version = 2") {
        Ok(CONTAINERD_V2_CRI_PLUGIN_ID)
    } else {
        Ok(CONTAINERD_LEGACY_CRI_PLUGIN_ID)
    }
}

/// True when the containerd config is v3 (version = 3), i.e. we use the split CRI plugins.
fn is_containerd_v3_config(pluginid: &str) -> bool {
    pluginid == CONTAINERD_V3_RUNTIME_PLUGIN_ID
}

fn get_containerd_output_path(paths: &ContainerdPaths) -> PathBuf {
    if paths.use_drop_in {
        if paths.drop_in_file.starts_with("/etc/containerd/") {
            Path::new(&paths.drop_in_file).to_path_buf()
        } else {
            let drop_in_path = paths.drop_in_file.trim_start_matches('/');
            Path::new("/host").join(drop_in_path)
        }
    } else {
        Path::new(&paths.config_file).to_path_buf()
    }
}

fn write_containerd_runtime_config(
    config_file: &Path,
    pluginid: &str,
    params: &ContainerdRuntimeParams,
) -> Result<()> {
    let runtime_table = format!(
        ".plugins.{}.containerd.runtimes.{}",
        pluginid, params.runtime_name
    );
    let runtime_options_table = format!("{runtime_table}.options");
    let runtime_type = format!("\"io.containerd.{}.v2\"", params.runtime_name);

    toml_utils::set_toml_value(
        config_file,
        &format!("{runtime_table}.runtime_type"),
        &runtime_type,
    )?;
    toml_utils::set_toml_value(
        config_file,
        &format!("{runtime_table}.runtime_path"),
        &params.runtime_path,
    )?;
    toml_utils::set_toml_value(
        config_file,
        &format!("{runtime_table}.privileged_without_host_devices"),
        "true",
    )?;
    toml_utils::set_toml_value(
        config_file,
        &format!("{runtime_table}.pod_annotations"),
        params.pod_annotations,
    )?;
    toml_utils::set_toml_value(
        config_file,
        &format!("{runtime_options_table}.ConfigPath"),
        &params.config_path,
    )?;

    if let Some(ref snapshotter) = params.snapshotter {
        toml_utils::set_toml_value(
            config_file,
            &format!("{runtime_table}.snapshotter"),
            snapshotter,
        )?;
        // In containerd config v3 the CRI plugin is split into runtime and images,
        // and setting the snapshotter only on the runtime plugin is not enough for image
        // pull/prepare.
        //
        // The images plugin must have runtime_platforms.<runtime>.snapshotter so it
        // uses the correct snapshotter per runtime (e.g. nydus, erofs).
        //
        // A PR on the containerd side is open so we can rely on the runtime plugin
        // snapshotter alone: https://github.com/containerd/containerd/pull/12836
        if is_containerd_v3_config(pluginid) {
            toml_utils::set_toml_value(
                config_file,
                &format!(
                    ".plugins.{}.runtime_platforms.\"{}\".snapshotter",
                    CONTAINERD_CRI_IMAGES_PLUGIN_ID,
                    params.runtime_name
                ),
                snapshotter,
            )?;
        }
    }

    Ok(())
}

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

    let paths = config.get_containerd_paths(runtime).await?;
    let configuration_file = get_containerd_output_path(&paths);
    let pluginid = match paths.plugin_id.as_deref() {
        Some(plugin_id) => plugin_id,
        None => get_containerd_pluginid(&paths.config_file)?,
    };

    log::info!(
        "configure_containerd_runtime: Writing to {:?}, pluginid={}",
        configuration_file,
        pluginid
    );

    let pod_annotations = "[\"io.katacontainers.*\"]";

    // Determine snapshotter if configured
    let snapshotter = config
        .snapshotter_handler_mapping_for_arch
        .as_ref()
        .and_then(|mapping| {
            mapping.split(',').find_map(|m| {
                let parts: Vec<&str> = m.split(':').collect();
                if parts.len() == 2 && parts[0] == shim {
                    let value = parts[1];
                    let snapshotter_value = if value == "nydus" {
                        match config.multi_install_suffix.as_ref() {
                            Some(suffix) if !suffix.is_empty() => format!("\"{value}-{suffix}\""),
                            _ => format!("\"{value}\""),
                        }
                    } else {
                        format!("\"{value}\"")
                    };
                    Some(snapshotter_value)
                } else {
                    None
                }
            })
        });

    let params = ContainerdRuntimeParams {
        runtime_name,
        runtime_path: format!(
            "\"{}\"",
            utils::get_kata_containers_runtime_path(shim, &config.dest_dir)
        ),
        config_path: format!(
            "\"{}/{}.toml\"",
            utils::get_kata_containers_config_path(shim, &config.dest_dir),
            configuration
        ),
        pod_annotations,
        snapshotter,
    };

    write_containerd_runtime_config(&configuration_file, pluginid, &params)?;

    if config.debug {
        toml_utils::set_toml_value(&configuration_file, ".debug.level", "\"debug\"")?;
    }

    Ok(())
}

/// Custom runtimes use an isolated config directory under custom-runtimes/{handler}/
pub async fn configure_custom_containerd_runtime(
    config: &Config,
    runtime: &str,
    custom_runtime: &CustomRuntime,
) -> Result<()> {
    log::info!(
        "configure_custom_containerd_runtime: Starting for handler={}",
        custom_runtime.handler
    );

    let paths = config.get_containerd_paths(runtime).await?;
    let configuration_file = get_containerd_output_path(&paths);
    let pluginid = match paths.plugin_id.as_deref() {
        Some(plugin_id) => plugin_id,
        None => get_containerd_pluginid(&paths.config_file)?,
    };

    log::info!(
        "configure_custom_containerd_runtime: Writing to {:?}, pluginid={}",
        configuration_file,
        pluginid
    );

    let pod_annotations = "[\"io.katacontainers.*\"]";

    // Determine snapshotter if specified
    let snapshotter = custom_runtime.containerd_snapshotter.as_ref().map(|s| {
        if s == "nydus" {
            match config.multi_install_suffix.as_ref() {
                Some(suffix) if !suffix.is_empty() => format!("\"{s}-{suffix}\""),
                _ => format!("\"{s}\""),
            }
        } else {
            format!("\"{s}\"")
        }
    });

    let params = ContainerdRuntimeParams {
        runtime_name: custom_runtime.handler.clone(),
        runtime_path: format!(
            "\"{}\"",
            utils::get_kata_containers_runtime_path(&custom_runtime.base_config, &config.dest_dir)
        ),
        config_path: format!(
            "\"{}/share/defaults/kata-containers/custom-runtimes/{}/configuration-{}.toml\"",
            config.dest_dir,
            custom_runtime.handler,
            custom_runtime.base_config
        ),
        pod_annotations,
        snapshotter,
    };

    write_containerd_runtime_config(&configuration_file, pluginid, &params)?;

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

    if config.custom_runtimes_enabled {
        if config.custom_runtimes.is_empty() {
            anyhow::bail!(
                "Custom runtimes enabled but no custom runtimes found in configuration. \
                 Check that custom-runtimes.list exists and is readable."
            );
        }
        log::info!(
            "Configuring {} custom runtime(s)",
            config.custom_runtimes.len()
        );
        for custom_runtime in &config.custom_runtimes {
            log::info!(
                "Configuring custom runtime: {}",
                custom_runtime.handler
            );
            configure_custom_containerd_runtime(config, runtime, custom_runtime).await?;
            log::info!(
                "Successfully configured custom runtime: {}",
                custom_runtime.handler
            );
        }
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

/// Setup containerd config files based on runtime type.
/// For K3s/RKE2, resolves which template (v2 or v3) to use from the node's containerd version,
/// then creates only that template file.
pub async fn setup_containerd_config_files(runtime: &str, config: &Config) -> Result<()> {
    const K3S_RKE2_BASE_TMPL: &str = "{{ template \"base\" . }}\n";

    match runtime {
        "k3s" | "k3s-agent" | "rke2-agent" | "rke2-server" => {
            // K3s/RKE2: create only the chosen template (v2 or v3). See docs.k3s.io/advanced#configuring-containerd
            let paths = config.get_containerd_paths(runtime).await?;
            let path = &paths.config_file;
            if !Path::new(path).exists() {
                if let Some(parent) = Path::new(path).parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("Failed to create containerd config dir: {parent:?}"))?;
                }
                fs::write(path, K3S_RKE2_BASE_TMPL)
                    .with_context(|| format!("Failed to write K3s/RKE2 template: {path}"))?;
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
    use crate::utils::toml as toml_utils;
    use rstest::rstest;
    use std::path::Path;
    use tempfile::NamedTempFile;

    fn make_params(
        runtime_name: &str,
        snapshotter: Option<&str>,
    ) -> ContainerdRuntimeParams {
        ContainerdRuntimeParams {
            runtime_name: runtime_name.to_string(),
            runtime_path: "\"/opt/kata/bin/kata-runtime\"".to_string(),
            config_path: "\"/opt/kata/share/defaults/kata-containers/configuration-qemu.toml\""
                .to_string(),
            pod_annotations: "[\"io.katacontainers.*\"]",
            snapshotter: snapshotter.map(|s| s.to_string()),
        }
    }

    /// CRI images runtime_platforms snapshotter is set only for v3 config when a snapshotter is configured.
    #[rstest]
    #[case(CONTAINERD_V3_RUNTIME_PLUGIN_ID, Some("\"nydus\""), "kata-qemu", true)]
    #[case(CONTAINERD_V2_CRI_PLUGIN_ID, Some("\"nydus\""), "kata-qemu", false)]
    #[case(CONTAINERD_V3_RUNTIME_PLUGIN_ID, None, "kata-qemu", false)]
    #[case(CONTAINERD_V3_RUNTIME_PLUGIN_ID, Some("\"erofs\""), "kata-clh", true)]
    fn test_write_containerd_runtime_config_cri_images_runtime_platforms_snapshotter(
        #[case] pluginid: &str,
        #[case] snapshotter: Option<&str>,
        #[case] runtime_name: &str,
        #[case] expect_runtime_platforms_set: bool,
    ) {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "").unwrap();

        let params = make_params(runtime_name, snapshotter);
        write_containerd_runtime_config(path, pluginid, &params).unwrap();

        let images_snapshotter_path = format!(
            ".plugins.\"io.containerd.cri.v1.images\".runtime_platforms.\"{}\".snapshotter",
            runtime_name
        );
        let result = toml_utils::get_toml_value(Path::new(path), &images_snapshotter_path);

        if expect_runtime_platforms_set {
            let value = result.unwrap_or_else(|e| {
                panic!(
                    "expected CRI images runtime_platforms.{} snapshotter to be set: {}",
                    runtime_name, e
                )
            });
            assert_eq!(
                value,
                snapshotter.unwrap().trim_matches('"'),
                "runtime_platforms snapshotter value"
            );
        } else {
            assert!(
                result.is_err(),
                "expected CRI images runtime_platforms.{} snapshotter not to be set for pluginid={:?} snapshotter={:?}",
                runtime_name,
                pluginid,
                snapshotter
            );
        }
    }

    /// Written containerd config (e.g. drop-in) must not start with blank lines when written to an initially empty file.
    #[rstest]
    #[case(CONTAINERD_V3_RUNTIME_PLUGIN_ID)]
    #[case(CONTAINERD_V2_CRI_PLUGIN_ID)]
    fn test_write_containerd_runtime_config_empty_file_no_leading_newlines(
        #[case] pluginid: &str,
    ) {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "").unwrap();

        let params = make_params("kata-qemu", Some("\"nydus\""));
        write_containerd_runtime_config(path, pluginid, &params).unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert!(
            !content.starts_with('\n'),
            "containerd config must not start with newline(s), got {} leading newlines (pluginid={})",
            content.chars().take_while(|&c| c == '\n').count(),
            pluginid
        );
        assert!(
            content.trim_start().starts_with('['),
            "config should start with a TOML table"
        );
    }

    #[rstest]
    #[case("containerd://1.6.28", true, false, Some("kata-deploy only supports snapshotter configuration with containerd 1.7 or newer"))]
    #[case("containerd://1.6.28", false, true, None)]
    #[case("containerd://1.6.0", true, false, None)]
    #[case("containerd://1.6.999", true, false, None)]
    #[case("containerd://1.7.0", true, true, None)]
    #[case("containerd://1.7.15", true, true, None)]
    #[case("containerd://1.8.0", true, true, None)]
    #[case("containerd://2.0.0", true, true, None)]
    #[case("1.6.28", true, false, None)]
    fn test_check_containerd_snapshotter_version_support(
        #[case] version: &str,
        #[case] has_mapping: bool,
        #[case] expect_ok: bool,
        #[case] expected_error_substring: Option<&str>,
    ) {
        let result = check_containerd_snapshotter_version_support(version, has_mapping);
        if expect_ok {
            assert!(result.is_ok(), "expected ok for version={} has_mapping={}", version, has_mapping);
        } else {
            assert!(result.is_err(), "expected err for version={} has_mapping={}", version, has_mapping);
            if let Some(sub) = expected_error_substring {
                assert!(
                    result.unwrap_err().to_string().contains(sub),
                    "error should contain {:?}",
                    sub
                );
            }
        }
    }

    #[rstest]
    #[case("containerd://2.2.0")]
    #[case("containerd://2.2.0-rc.1")]
    #[case("containerd://2.2.1")]
    #[case("containerd://2.3.0")]
    #[case("containerd://3.0.0")]
    #[case("containerd://2.3.0-beta.0")]
    #[case("2.2.0")]
    fn test_check_containerd_erofs_version_support_passing(#[case] version: &str) {
        assert!(
            check_containerd_erofs_version_support(version).is_ok(),
            "Expected {} to pass",
            version
        );
    }

    #[rstest]
    #[case("containerd://2.1.0", "containerd must be 2.2.0 or newer")]
    #[case("containerd://2.1.5-rc.1", "containerd must be 2.2.0 or newer")]
    #[case("containerd://2.0.0", "containerd must be 2.2.0 or newer")]
    #[case("containerd://1.7.0", "containerd must be 2.2.0 or newer")]
    #[case("containerd://1.6.28", "containerd must be 2.2.0 or newer")]
    #[case("2.1.0", "containerd must be 2.2.0 or newer")]
    #[case("invalid", "Invalid containerd version format")]
    #[case("containerd://abc.2.0", "Failed to parse major version")]
    fn test_check_containerd_erofs_version_support_failing(
        #[case] version: &str,
        #[case] expected_error: &str,
    ) {
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
