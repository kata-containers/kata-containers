// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Config, DEFAULT_KATA_INSTALL_DIR};
use crate::k8s::nfd;
use crate::k8s::runtimeclasses;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::{Context, Result};
use log::info;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use walkdir::WalkDir;

/// All valid shims
const ALL_SHIMS: &[&str] = &[
    // Non-QEMU shims
    "clh",
    "cloud-hypervisor",
    "dragonball",
    "fc",
    "firecracker",
    "remote",
    // QEMU shims
    "qemu",
    "qemu-cca",
    "qemu-coco-dev",
    "qemu-coco-dev-runtime-rs",
    "qemu-nvidia-gpu",
    "qemu-nvidia-gpu-snp",
    "qemu-nvidia-gpu-tdx",
    "qemu-runtime-rs",
    "qemu-se",
    "qemu-se-runtime-rs",
    "qemu-snp",
    "qemu-snp-runtime-rs",
    "qemu-tdx",
    "qemu-tdx-runtime-rs",
];

/// Check if a shim is a QEMU-based shim (all QEMU shims start with "qemu")
fn is_qemu_shim(shim: &str) -> bool {
    shim.starts_with("qemu")
}

/// Get all valid shim names as a comma-separated string for error messages
fn get_all_valid_shims() -> String {
    ALL_SHIMS.join(", ")
}

/// Get hypervisor name from shim name
fn get_hypervisor_name(shim: &str) -> Result<&str> {
    if is_qemu_shim(shim) {
        return Ok("qemu");
    }

    match shim {
        "clh" => Ok("clh"),
        "cloud-hypervisor" => Ok("cloud-hypervisor"),
        "dragonball" => Ok("dragonball"),
        "fc" | "firecracker" => Ok("firecracker"),
        "remote" => Ok("remote"),
        _ => anyhow::bail!(
            "Unknown shim '{}'. Valid shims are: {}",
            shim,
            get_all_valid_shims()
        ),
    }
}

pub async fn install_artifacts(config: &Config) -> Result<()> {
    info!("copying kata artifacts onto host");

    // Create the installation directory if it doesn't exist
    // fs::create_dir_all handles existing directories gracefully (returns Ok if already exists)
    fs::create_dir_all(&config.host_install_dir)
        .with_context(|| format!("Failed to create installation directory: {}", config.host_install_dir))?;

    // Verify the path exists and is a directory (not a file)
    let install_path = Path::new(&config.host_install_dir);
    if !install_path.exists() {
        return Err(anyhow::anyhow!(
            "Installation directory does not exist after creation: {}",
            config.host_install_dir
        ));
    }
    if !install_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Installation path exists but is not a directory: {}",
            config.host_install_dir
        ));
    }

    copy_artifacts("/opt/kata-artifacts/opt/kata", &config.host_install_dir)?;

    set_executable_permissions(&config.host_install_dir)?;

    for shim in &config.shims_for_arch {
        configure_shim_config(config, shim).await?;
    }

    // Install custom runtime configuration files if enabled
    if config.custom_runtimes_enabled && !config.custom_runtimes.is_empty() {
        install_custom_runtime_configs(config)?;
    }

    if std::env::var("HOST_OS").unwrap_or_default() == "cbl-mariner" {
        configure_mariner(config).await?;
    }

    let expand_runtime_classes_for_nfd = nfd::setup_nfd_rules(config).await?;

    if expand_runtime_classes_for_nfd {
        runtimeclasses::update_existing_runtimeclasses_for_nfd(config).await?;
    }

    Ok(())
}

pub async fn remove_artifacts(config: &Config) -> Result<()> {
    info!("deleting kata artifacts");

    // Remove runtime directories for each shim (drop-in configs, symlinks)
    for shim in &config.shims_for_arch {
        if let Err(e) = remove_runtime_directory(config, shim) {
            log::warn!("Failed to remove runtime directory for {}: {}", shim, e);
        }
    }

    // Remove custom runtime configs (before removing main install dir)
    if config.custom_runtimes_enabled && !config.custom_runtimes.is_empty() {
        remove_custom_runtime_configs(config)?;
    }

    if Path::new(&config.host_install_dir).exists() {
        fs::remove_dir_all(&config.host_install_dir)?;
    }

    nfd::remove_nfd_rules(config).await?;

    Ok(())
}

/// Each custom runtime gets an isolated directory under custom-runtimes/{handler}/
fn install_custom_runtime_configs(config: &Config) -> Result<()> {
    info!("Installing custom runtime configuration files");

    for runtime in &config.custom_runtimes {
        // Create isolated directory for this handler
        let handler_dir = format!(
            "/host/{}/share/defaults/kata-containers/custom-runtimes/{}",
            config.dest_dir, runtime.handler
        );
        let config_d_dir = format!("{}/config.d", handler_dir);

        fs::create_dir_all(&config_d_dir)
            .with_context(|| format!("Failed to create config.d directory: {}", config_d_dir))?;

        // Copy base config to the handler directory
        // Custom runtime drop-ins will overlay on top of this
        let base_config_filename = format!("configuration-{}.toml", runtime.base_config);
        let original_config = format!(
            "/host/{}/share/defaults/kata-containers/{}",
            config.dest_dir, base_config_filename
        );
        let dest_config = format!("{}/{}", handler_dir, base_config_filename);

        if Path::new(&original_config).exists() {
            // Remove existing destination (might be a symlink from older versions)
            let dest_path = Path::new(&dest_config);
            if dest_path.exists() || dest_path.is_symlink() {
                fs::remove_file(&dest_config).with_context(|| {
                    format!("Failed to remove existing config: {}", dest_config)
                })?;
            }

            fs::copy(&original_config, &dest_config).with_context(|| {
                format!(
                    "Failed to copy config: {} -> {}",
                    original_config, dest_config
                )
            })?;

            info!(
                "Copied config for custom runtime {}: {} -> {}",
                runtime.handler, original_config, dest_config
            );
        }

        // Copy drop-in file if provided
        if let Some(ref drop_in_src) = runtime.drop_in_file {
            let drop_in_dest = format!("{}/50-overrides.toml", config_d_dir);

            info!(
                "Copying drop-in for {}: {} -> {}",
                runtime.handler, drop_in_src, drop_in_dest
            );

            fs::copy(drop_in_src, &drop_in_dest).with_context(|| {
                format!(
                    "Failed to copy drop-in from {} to {}",
                    drop_in_src, drop_in_dest
                )
            })?;
        }
    }

    info!(
        "Successfully installed {} custom runtime config(s)",
        config.custom_runtimes.len()
    );
    Ok(())
}

fn remove_custom_runtime_configs(config: &Config) -> Result<()> {
    info!("Removing custom runtime configuration files");

    let custom_runtimes_dir = format!(
        "/host/{}/share/defaults/kata-containers/custom-runtimes",
        config.dest_dir
    );

    for runtime in &config.custom_runtimes {
        // Remove the entire handler directory (includes config.d/)
        let handler_dir = format!("{}/{}", custom_runtimes_dir, runtime.handler);

        if Path::new(&handler_dir).exists() {
            info!("Removing custom runtime directory: {}", handler_dir);
            if let Err(e) = fs::remove_dir_all(&handler_dir) {
                log::warn!(
                    "Failed to remove custom runtime directory {}: {}",
                    handler_dir,
                    e
                );
            }
        }
    }

    // Remove the custom-runtimes directory if empty
    if Path::new(&custom_runtimes_dir).exists() {
        if let Ok(entries) = fs::read_dir(&custom_runtimes_dir) {
            if entries.count() == 0 {
                let _ = fs::remove_dir(&custom_runtimes_dir);
            }
        }
    }

    info!("Successfully removed custom runtime config files");
    Ok(())
}

/// Note: The src parameter is kept to allow for unit testing with temporary directories,
/// even though in production it always uses /opt/kata-artifacts/opt/kata
fn copy_artifacts(src: &str, dst: &str) -> Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let src_path = entry.path();
        let relative_path = src_path.strip_prefix(src)?;
        let dst_path = Path::new(dst).join(relative_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Remove destination file first (ignore if it doesn't exist)
            // This is crucial for atomic updates:
            // - If the file is in use (e.g., a running binary), the old inode stays alive
            // - The new copy creates a new inode
            // - Running processes keep using the old inode until they exit
            // - New processes use the new file immediately
            match fs::remove_file(&dst_path) {
                Ok(()) => {}                                             // File removed successfully
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // File didn't exist, that's fine
                Err(e) => return Err(e.into()), // Other errors should be propagated
            }

            fs::copy(src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn set_executable_permissions(dir: &str) -> Result<()> {
    let bin_paths = vec!["bin", "runtime-rs/bin"];
    
    for bin_path in bin_paths.iter() {
        let bin_dir = Path::new(dir).join(bin_path);
        if bin_dir.exists() {
            for entry in fs::read_dir(&bin_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let mut perms = fs::metadata(&path)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&path, perms)?;
                }
            }
        }
    }

    Ok(())
}

/// Set up the runtime directory structure for a shim.
/// Creates: {config_path}/runtimes/{shim}/
///          {config_path}/runtimes/{shim}/config.d/
///          {config_path}/runtimes/{shim}/configuration-{shim}.toml (copy of original)
///
/// Note: We copy the config file instead of symlinking because kata-containers'
/// ResolvePath uses filepath.EvalSymlinks, which would resolve to the original
/// location and look for config.d there instead of in our per-shim directory.
fn setup_runtime_directory(config: &Config, shim: &str) -> Result<()> {
    let original_config_dir = format!(
        "/host{}",
        utils::get_kata_containers_original_config_path(shim, &config.dest_dir)
    );
    let runtime_config_dir = format!(
        "/host{}",
        utils::get_kata_containers_config_path(shim, &config.dest_dir)
    );
    let config_d_dir = format!("{}/config.d", runtime_config_dir);

    // Create the runtime directory and config.d subdirectory
    fs::create_dir_all(&config_d_dir)
        .with_context(|| format!("Failed to create config.d directory: {}", config_d_dir))?;

    // Copy the original config file to the runtime directory
    let original_config_file = format!("{}/configuration-{}.toml", original_config_dir, shim);
    let dest_config_file = format!("{}/configuration-{}.toml", runtime_config_dir, shim);

    // Only copy if original exists
    if Path::new(&original_config_file).exists() {
        // Remove existing destination (might be a symlink from older versions)
        // fs::copy follows symlinks and would write to the wrong location
        let dest_path = Path::new(&dest_config_file);
        if dest_path.exists() || dest_path.is_symlink() {
            fs::remove_file(&dest_config_file)
                .with_context(|| format!("Failed to remove existing config: {}", dest_config_file))?;
        }

        // Copy the base config file
        fs::copy(&original_config_file, &dest_config_file)
            .with_context(|| format!("Failed to copy config: {} -> {}", original_config_file, dest_config_file))?;
        
        log::debug!(
            "Copied config for {}: {} -> {}",
            shim,
            original_config_file,
            dest_config_file
        );
    }

    Ok(())
}

/// Remove the runtime directory for a shim during cleanup
fn remove_runtime_directory(config: &Config, shim: &str) -> Result<()> {
    let runtime_config_dir = format!(
        "/host{}",
        utils::get_kata_containers_config_path(shim, &config.dest_dir)
    );

    if Path::new(&runtime_config_dir).exists() {
        fs::remove_dir_all(&runtime_config_dir)
            .with_context(|| format!("Failed to remove runtime directory: {}", runtime_config_dir))?;
        log::debug!("Removed runtime directory: {}", runtime_config_dir);
    }

    // Try to clean up parent 'runtimes' directory if empty
    let runtimes_dir = Path::new(&runtime_config_dir).parent();
    if let Some(runtimes_path) = runtimes_dir {
        if runtimes_path.exists() {
            if let Ok(entries) = fs::read_dir(runtimes_path) {
                if entries.count() == 0 {
                    let _ = fs::remove_dir(runtimes_path);
                }
            }
        }
    }

    Ok(())
}

async fn configure_shim_config(config: &Config, shim: &str) -> Result<()> {
    // Set up the runtime directory structure with symlink to original config
    setup_runtime_directory(config, shim)?;

    let runtime_config_dir = format!(
        "/host{}",
        utils::get_kata_containers_config_path(shim, &config.dest_dir)
    );
    let config_d_dir = format!("{}/config.d", runtime_config_dir);

    let kata_config_file = Path::new(&runtime_config_dir).join(format!("configuration-{shim}.toml"));

    // The configuration file (symlink) should exist after setup_runtime_directory()
    if !kata_config_file.exists() {
        return Err(anyhow::anyhow!(
            "Configuration file not found: {kata_config_file:?}. This file should have been \
             symlinked from the original config. Check that the shim '{}' has a valid configuration \
             file in the artifacts.",
            shim
        ));
    }

    // 1. Installation prefix adjustments (if not default)
    if config.dest_dir != DEFAULT_KATA_INSTALL_DIR {
        let prefix_content = generate_installation_prefix_drop_in(config, shim)?;
        write_drop_in_file(&config_d_dir, "10-installation-prefix.toml", &prefix_content)?;
    }

    // 2. Debug configuration (boolean flags only via drop-in)
    // kernel_params for debug will be handled by the combined kernel_params drop-in
    if config.debug {
        let debug_content = generate_debug_drop_in(shim)?;
        write_drop_in_file(&config_d_dir, "20-debug.toml", &debug_content)?;
    }

    configure_proxy(config, shim, &kata_config_file, "https_proxy").await?;

    configure_no_proxy(config, shim, &kata_config_file).await?;

    if config.debug {
        configure_debug(&kata_config_file, shim).await?;
    }

    configure_hypervisor_annotations(config, shim, &kata_config_file).await?;

    if config
        .experimental_force_guest_pull_for_arch
        .contains(&shim.to_string())
    {
        configure_experimental_force_guest_pull(&kata_config_file).await?;
    }

    Ok(())
}

fn update_kernel_param(current_params: &str, param_name: &str, param_value: &str) -> String {
    let full_param = format!("{param_name}={param_value}");
    let search_prefix = format!("{param_name}=");

    // Split params by whitespace and process each
    let params: Vec<&str> = current_params.split_whitespace().collect();
    let mut updated_params: Vec<String> = Vec::new();
    let mut found = false;

    for param in params {
        if param.starts_with(&search_prefix) {
            // Replace existing parameter with new value
            updated_params.push(full_param.clone());
            found = true;
        } else {
            updated_params.push(param.to_string());
        }
    }

    // If parameter wasn't found, append it
    if !found {
        updated_params.push(full_param);
    }

    updated_params.join(" ")
}

async fn configure_proxy(
    config: &Config,
    shim: &str,
    config_file: &Path,
    proxy_type: &str,
) -> Result<()> {
    let proxy_var = if proxy_type == "https_proxy" {
        &config.agent_https_proxy
    } else {
        return Ok(());
    };

    let proxy_value = match proxy_var {
        Some(proxy) if !proxy.is_empty() && proxy.contains('=') => {
            // Per-shim format: "qemu-tdx=http://proxy:8080;qemu-snp=http://proxy2:8080"
            proxy
                .split(';')
                .find_map(|m| {
                    let parts: Vec<&str> = m.splitn(2, '=').collect();
                    if parts.len() == 2 && parts[0] == shim {
                        Some(parts[1].to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
        }
        Some(proxy) if !proxy.is_empty() => proxy.clone(),
        _ => return Ok(()),
    };

    if !proxy_value.is_empty() {
        let hypervisor_name = get_hypervisor_name(shim)?;
        let kernel_params_path = format!("hypervisor.{hypervisor_name}.kernel_params");
        let param_name = "agent.https_proxy";

        // Get current kernel_params and update/append the proxy setting
        let current_params =
            toml_utils::get_toml_value(config_file, &kernel_params_path).unwrap_or_default();

        let updated_params = update_kernel_param(&current_params, param_name, &proxy_value);

        log::debug!(
            "Updating {} in {}: old=\"{}\" new=\"{}\"",
            kernel_params_path,
            config_file.display(),
            current_params,
            updated_params
        );

        // Set the updated kernel_params (replace entire value)
        toml_utils::set_toml_value(
            config_file,
            &kernel_params_path,
            &format!("\"{}\"", updated_params),
        )?;
    }

    Ok(())
}

async fn configure_no_proxy(config: &Config, shim: &str, config_file: &Path) -> Result<()> {
    let no_proxy_value = match &config.agent_no_proxy {
        Some(no_proxy) if !no_proxy.is_empty() && no_proxy.contains('=') => {
            // Per-shim format
            no_proxy
                .split(';')
                .find_map(|m| {
                    let parts: Vec<&str> = m.splitn(2, '=').collect();
                    if parts.len() == 2 && parts[0] == shim {
                        Some(parts[1].to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
        }
        Some(no_proxy) if !no_proxy.is_empty() => no_proxy.clone(),
        _ => return Ok(()),
    };

    if !no_proxy_value.is_empty() {
        let hypervisor_name = get_hypervisor_name(shim)?;
        let kernel_params_path = format!("hypervisor.{hypervisor_name}.kernel_params");

        // Get current kernel_params and update/append the no_proxy setting
        let current_params =
            toml_utils::get_toml_value(config_file, &kernel_params_path).unwrap_or_default();

        let updated_params =
            update_kernel_param(&current_params, "agent.no_proxy", &no_proxy_value);

        log::debug!(
            "Updating {} in {}: old=\"{}\" new=\"{}\"",
            kernel_params_path,
            config_file.display(),
            current_params,
            updated_params
        );

        // Set the updated kernel_params (replace entire value)
        toml_utils::set_toml_value(
            config_file,
            &kernel_params_path,
            &format!("\"{}\"", updated_params),
        )?;
    }

    Ok(())
}

/// Set a TOML boolean value to "true" if it's not already "true"
/// Reads the current value (defaulting to "false" if not found), and if it's not "true",
/// logs the update and sets it to "true".
fn set_toml_bool_to_true(config_file: &Path, path: &str) -> Result<()> {
    let current_value = toml_utils::get_toml_value(config_file, path)
        .unwrap_or_else(|_| "false".to_string());
    if current_value != "true" {
        log::debug!(
            "Updating {} in {}: old=\"{}\" new=\"true\"",
            path,
            config_file.display(),
            current_value
        );
        toml_utils::set_toml_value(config_file, path, "true")?;
    }
    Ok(())
}

/// Write a drop-in configuration file to the config.d directory.
/// If content is empty, the file is not created.
fn write_drop_in_file(config_d_dir: &str, filename: &str, content: &str) -> Result<()> {
    if content.is_empty() {
        return Ok(());
    }

    let drop_in_path = format!("{}/{}", config_d_dir, filename);
    fs::write(&drop_in_path, content)
        .with_context(|| format!("Failed to write drop-in file: {}", drop_in_path))?;

    log::debug!("Created drop-in file: {}", drop_in_path);
    Ok(())
}

/// Get the QEMU share directory name for a given shim.
/// Some shims use experimental QEMU builds with different firmware paths.
fn get_qemu_share_name(shim: &str) -> Option<String> {
    if !is_qemu_shim(shim) {
        return None;
    }

    let share_name = match shim {
        "qemu-cca" => "qemu-cca-experimental",
        "qemu-nvidia-gpu-snp" => "qemu-snp-experimental",
        "qemu-nvidia-gpu-tdx" => "qemu-tdx-experimental",
        _ => "qemu",
    };

    Some(share_name.to_string())
}

/// Create a QEMU wrapper script that adds the -L flag for firmware paths.
/// This is needed when using a non-default installation prefix.
fn create_qemu_wrapper_script(config: &Config, shim: &str) -> Result<Option<String>> {
    let qemu_share = match get_qemu_share_name(shim) {
        Some(share) => share,
        None => return Ok(None), // Not a QEMU shim, no wrapper needed
    };

    let qemu_binary = format!("{}/bin/qemu-system-x86_64", config.dest_dir);
    let wrapper_script_path = format!("{}-installation-prefix", qemu_binary);
    let host_wrapper_path = format!("/host{}", wrapper_script_path);

    // Create wrapper script if it doesn't exist
    if !Path::new(&host_wrapper_path).exists() {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(&host_wrapper_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let script_content = format!(
            r#"#!/usr/bin/env bash

exec {} "$@" -L {}/share/kata-{}/qemu/
"#,
            qemu_binary, config.dest_dir, qemu_share
        );

        fs::write(&host_wrapper_path, &script_content)?;
        let mut perms = fs::metadata(&host_wrapper_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&host_wrapper_path, perms)?;

        log::debug!("Created QEMU wrapper script: {}", host_wrapper_path);
    }

    Ok(Some(wrapper_script_path))
}

/// Get the hypervisor binary path for a given shim.
/// Returns the path to the hypervisor binary based on the shim type.
fn get_hypervisor_path(config: &Config, shim: &str) -> Result<String> {
    if is_qemu_shim(shim) {
        // For QEMU shims, use the wrapper script that adds firmware paths
        // create_qemu_wrapper_script always returns Some for QEMU shims
        create_qemu_wrapper_script(config, shim)?
            .ok_or_else(|| anyhow::anyhow!("QEMU wrapper script should always be created for QEMU shims"))
    } else {
        // For non-QEMU shims, use the appropriate hypervisor binary
        let binary = match shim {
            "clh" | "cloud-hypervisor" => "cloud-hypervisor",
            "fc" | "firecracker" => "firecracker",
            "dragonball" => "dragonball",
            "stratovirt" => "stratovirt",
            // Remote and other shims don't have a local hypervisor binary
            _ => return Ok(String::new()),
        };
        Ok(format!("{}/bin/{}", config.dest_dir, binary))
    }
}

/// Generate drop-in content for installation prefix adjustments.
/// This replaces /opt/kata with the custom dest_dir in all relevant paths.
/// For QEMU shims, this also creates a wrapper script for firmware paths.
fn generate_installation_prefix_drop_in(config: &Config, shim: &str) -> Result<String> {
    let hypervisor_name = get_hypervisor_name(shim)?;

    // Build the drop-in content with adjusted paths
    let mut content = String::new();
    content.push_str("# Installation prefix adjustments\n");
    content.push_str("# Generated by kata-deploy\n\n");

    // Hypervisor section
    content.push_str(&format!("[hypervisor.{}]\n", hypervisor_name));

    // Only set hypervisor path if applicable for this shim type
    let hypervisor_path = get_hypervisor_path(config, shim)?;
    if !hypervisor_path.is_empty() {
        content.push_str(&format!("path = \"{}\"\n", hypervisor_path));
    }

    // Common paths for all hypervisors
    content.push_str(&format!("kernel = \"{}/share/kata-containers/vmlinux.container\"\n", config.dest_dir));
    content.push_str(&format!("image = \"{}/share/kata-containers/kata-containers.img\"\n", config.dest_dir));
    content.push_str(&format!("initrd = \"{}/share/kata-containers/kata-containers-initrd.img\"\n", config.dest_dir));

    // QEMU-specific paths (firmware is only relevant for QEMU)
    if is_qemu_shim(shim) {
        content.push_str(&format!("firmware = \"{}/share/kata-containers/firmware/\"\n", config.dest_dir));
        content.push_str(&format!("firmware_volume = \"{}/share/kata-containers/firmware/\"\n", config.dest_dir));
    }

    // Firecracker-specific paths (jailer is only for Firecracker)
    if shim == "fc" || shim == "firecracker" {
        content.push_str(&format!("jailer_path = \"{}/bin/jailer\"\n", config.dest_dir));
        content.push_str(&format!("valid_jailer_paths = [\"{}/bin/jailer\"]\n", config.dest_dir));
    }

    Ok(content)
}

/// Generate drop-in content for debug configuration.
/// Enables debug settings for the hypervisor, runtime, and agent.
/// Note: kernel_params for debug are handled separately in generate_kernel_params_drop_in
fn generate_debug_drop_in(shim: &str) -> Result<String> {
    let hypervisor_name = get_hypervisor_name(shim)?;

    let content = format!(
        r#"# Debug configuration
# Generated by kata-deploy

[hypervisor.{}]
enable_debug = true

[runtime]
enable_debug = true

[agent.kata]
debug_console_enabled = true
enable_debug = true
"#,
        hypervisor_name
    );

    Ok(content)
}

async fn configure_debug(config_file: &Path, shim: &str) -> Result<()> {
    let hypervisor_name = get_hypervisor_name(shim)?;

    let hypervisor_enable_debug_path = format!("hypervisor.{hypervisor_name}.enable_debug");
    set_toml_bool_to_true(config_file, &hypervisor_enable_debug_path)?;

    set_toml_bool_to_true(config_file, "runtime.enable_debug")?;

    set_toml_bool_to_true(config_file, "agent.kata.debug_console_enabled")?;

    set_toml_bool_to_true(config_file, "agent.kata.enable_debug")?;

    let kernel_params_path = format!("hypervisor.{hypervisor_name}.kernel_params");
    let current_params =
        toml_utils::get_toml_value(config_file, &kernel_params_path).unwrap_or_default();

    let mut debug_params = String::new();
    if !current_params.contains("agent.log=debug") {
        debug_params.push_str(" agent.log=debug");
    }
    if !current_params.contains("initcall_debug") {
        debug_params.push_str(" initcall_debug");
    }

    if !debug_params.is_empty() {
        let new_params = format!("{}{}", current_params, debug_params);
        log::debug!(
            "Updating {} in {}: old=\"{}\" new=\"{}\"",
            kernel_params_path,
            config_file.display(),
            current_params,
            new_params
        );
        toml_utils::append_to_toml_string(config_file, &kernel_params_path, debug_params.trim())?;
    }

    Ok(())
}

async fn configure_hypervisor_annotations(
    config: &Config,
    shim: &str,
    config_file: &Path,
) -> Result<()> {
    if config.allowed_hypervisor_annotations_for_arch.is_empty() {
        return Ok(());
    }

    let mut shim_specific_annotations = Vec::new();
    let mut global_annotations = Vec::new();

    for m in &config.allowed_hypervisor_annotations_for_arch {
        if m.contains(':') {
            // Shim-specific: "qemu:foo,bar"
            let parts: Vec<&str> = m.splitn(2, ':').collect();
            if parts.len() == 2 && parts[0] == shim {
                shim_specific_annotations.extend(parts[1].split(','));
            }
        } else {
            // Global: "foo bar"
            global_annotations.extend(m.split_whitespace());
        }
    }

    let mut all_annotations = global_annotations;
    all_annotations.extend(shim_specific_annotations);

    if all_annotations.is_empty() {
        return Ok(());
    }

    let hypervisor_name = get_hypervisor_name(shim)?;
    let enable_annotations_path = format!("hypervisor.{hypervisor_name}.enable_annotations");

    let existing = toml_utils::get_toml_array(config_file, &enable_annotations_path)
        .unwrap_or_else(|_| Vec::new());

    let mut combined: Vec<String> = existing.clone();

    combined.extend(all_annotations.iter().map(|s| s.trim().to_string()));

    combined.sort();
    combined.dedup();

    log::debug!(
        "Updating {} in {}: old={:?} new={:?}",
        enable_annotations_path,
        config_file.display(),
        existing,
        combined
    );

    toml_utils::set_toml_array(config_file, &enable_annotations_path, &combined)?;

    Ok(())
}

async fn configure_experimental_force_guest_pull(config_file: &Path) -> Result<()> {
    set_toml_bool_to_true(config_file, "runtime.experimental_force_guest_pull")
}

async fn configure_mariner(config: &Config) -> Result<()> {
    let config_path = format!(
        "{}/share/defaults/kata-containers/configuration-clh.toml",
        config.host_install_dir
    );
    let config_file = Path::new(&config_path);

    if !config_file.exists() {
        return Ok(());
    }

    let mariner_hypervisor_name = "clh";

    let static_resource_mgmt_path =
        format!("hypervisor.{mariner_hypervisor_name}.static_sandbox_resource_mgmt");
    set_toml_bool_to_true(config_file, &static_resource_mgmt_path)?;

    let clh_path = format!("{}/bin/cloud-hypervisor-glibc", config.dest_dir);
    let valid_paths_field = format!("hypervisor.{mariner_hypervisor_name}.valid_hypervisor_paths");
    let existing_paths =
        toml_utils::get_toml_array(config_file, &valid_paths_field).unwrap_or_else(|_| Vec::new());

    if !existing_paths.iter().any(|p| p == &clh_path) {
        let mut new_paths = existing_paths.clone();
        new_paths.push(clh_path.clone());
        log::debug!(
            "Updating {} in {}: old={:?} new={:?}",
            valid_paths_field,
            config_file.display(),
            existing_paths,
            new_paths
        );
        toml_utils::set_toml_array(config_file, &valid_paths_field, &new_paths)?;
    }

    let path_field = format!("hypervisor.{mariner_hypervisor_name}.path");
    let current_path = toml_utils::get_toml_value(config_file, &path_field).unwrap_or_default();
    if !current_path.contains(&clh_path) {
        log::debug!(
            "Updating {} in {}: old=\"{}\" new=\"{}\"",
            path_field,
            config_file.display(),
            current_path,
            clh_path
        );
        toml_utils::set_toml_value(config_file, &path_field, &format!("\"{clh_path}\""))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_hypervisor_name_qemu_variants() {
        // Test all QEMU variants
        assert_eq!(get_hypervisor_name("qemu").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-tdx").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-snp").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-se").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-coco-dev").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-cca").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-nvidia-gpu").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-nvidia-gpu-tdx").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-nvidia-gpu-snp").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-runtime-rs").unwrap(), "qemu");
        assert_eq!(
            get_hypervisor_name("qemu-coco-dev-runtime-rs").unwrap(),
            "qemu"
        );
        assert_eq!(get_hypervisor_name("qemu-se-runtime-rs").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-snp-runtime-rs").unwrap(), "qemu");
        assert_eq!(get_hypervisor_name("qemu-tdx-runtime-rs").unwrap(), "qemu");
    }

    #[test]
    fn test_get_hypervisor_name_other_hypervisors() {
        // Test other hypervisors
        assert_eq!(get_hypervisor_name("clh").unwrap(), "clh");
        assert_eq!(
            get_hypervisor_name("cloud-hypervisor").unwrap(),
            "cloud-hypervisor"
        );
        assert_eq!(get_hypervisor_name("dragonball").unwrap(), "dragonball");
        assert_eq!(get_hypervisor_name("fc").unwrap(), "firecracker");
        assert_eq!(get_hypervisor_name("firecracker").unwrap(), "firecracker");
        assert_eq!(get_hypervisor_name("remote").unwrap(), "remote");
    }

    #[test]
    fn test_get_hypervisor_name_unknown() {
        // Test unknown shim returns error with clear message
        let result = get_hypervisor_name("unknown-shim");
        assert!(result.is_err(), "Unknown shim should return an error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unknown shim 'unknown-shim'"),
            "Error message should mention the unknown shim"
        );
        assert!(
            err_msg.contains("Valid shims are:"),
            "Error message should list valid shims"
        );

        let result = get_hypervisor_name("custom");
        assert!(result.is_err(), "Custom shim should return an error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unknown shim 'custom'"),
            "Error message should mention the custom shim"
        );
    }

    #[test]
    fn test_copy_artifacts_overwrites_existing_files() {
        use std::fs;
        use tempfile::TempDir;

        // Create source directory with a file
        let src_dir = TempDir::new().unwrap();
        let src_file = src_dir.path().join("test.txt");
        fs::write(&src_file, "new content").unwrap();

        // Create destination directory with an existing file
        let dst_dir = TempDir::new().unwrap();
        let dst_file = dst_dir.path().join("test.txt");
        fs::write(&dst_file, "old content").unwrap();

        // Verify old content before copy
        let old_content = fs::read_to_string(&dst_file).unwrap();
        assert_eq!(old_content, "old content");

        // Copy (should remove old file first, then copy new one)
        copy_artifacts(
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
        )
        .unwrap();

        // Verify new content after copy
        let new_content = fs::read_to_string(&dst_file).unwrap();
        assert_eq!(new_content, "new content");

        // File should exist
        assert!(dst_file.exists());
    }

    #[test]
    fn test_copy_artifacts_preserves_structure() {
        use std::fs;
        use tempfile::TempDir;

        // Create source directory with nested structure
        let src_dir = TempDir::new().unwrap();
        let nested = src_dir.path().join("subdir");
        fs::create_dir(&nested).unwrap();
        fs::write(src_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(nested.join("file2.txt"), "content2").unwrap();

        // Copy to destination
        let dst_dir = TempDir::new().unwrap();
        copy_artifacts(
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
        )
        .unwrap();

        // Verify structure
        assert!(dst_dir.path().join("file1.txt").exists());
        assert!(dst_dir.path().join("subdir").exists());
        assert!(dst_dir.path().join("subdir/file2.txt").exists());

        // Verify content
        let content1 = fs::read_to_string(dst_dir.path().join("file1.txt")).unwrap();
        let content2 = fs::read_to_string(dst_dir.path().join("subdir/file2.txt")).unwrap();
        assert_eq!(content1, "content1");
        assert_eq!(content2, "content2");
    }

    #[test]
    fn test_update_kernel_param_append_new() {
        // Test appending a new parameter to empty params
        let current = "";
        let result = update_kernel_param(current, "agent.https_proxy", "http://proxy:8080");
        assert_eq!(result, "agent.https_proxy=http://proxy:8080");

        // Test appending to existing params
        let current = "console=ttyS0 agent.log=debug";
        let result = update_kernel_param(current, "agent.https_proxy", "http://proxy:8080");
        assert_eq!(
            result,
            "console=ttyS0 agent.log=debug agent.https_proxy=http://proxy:8080"
        );
    }

    #[test]
    fn test_update_kernel_param_replace_existing() {
        // Test replacing an existing parameter
        let current = "console=ttyS0 agent.https_proxy=http://old:8080 agent.log=debug";
        let result = update_kernel_param(current, "agent.https_proxy", "http://new:9090");
        assert_eq!(
            result,
            "console=ttyS0 agent.https_proxy=http://new:9090 agent.log=debug"
        );

        // Test replacing when it's the first parameter
        let current = "agent.https_proxy=http://old:8080 console=ttyS0";
        let result = update_kernel_param(current, "agent.https_proxy", "http://new:9090");
        assert_eq!(result, "agent.https_proxy=http://new:9090 console=ttyS0");

        // Test replacing when it's the last parameter
        let current = "console=ttyS0 agent.https_proxy=http://old:8080";
        let result = update_kernel_param(current, "agent.https_proxy", "http://new:9090");
        assert_eq!(result, "console=ttyS0 agent.https_proxy=http://new:9090");
    }

    #[test]
    fn test_update_kernel_param_with_duplicates() {
        // Test that we replace all occurrences when there are duplicates (avoid duplicates)
        let current = "agent.https_proxy=http://proxy1:8080 console=ttyS0 agent.https_proxy=http://proxy2:8080";
        let result = update_kernel_param(current, "agent.https_proxy", "http://new:9090");

        // Should replace the first occurrence and also replace any subsequent duplicates
        // This ensures we don't have multiple conflicting values for the same parameter
        assert_eq!(
            result,
            "agent.https_proxy=http://new:9090 console=ttyS0 agent.https_proxy=http://new:9090"
        );

        // Note: Having duplicate parameters in kernel_params is unusual, but if they exist,
        // we update all of them to the same new value to maintain consistency
    }

    #[test]
    fn test_update_kernel_param_complex_values() {
        // Test with URL containing special characters
        let current = "console=ttyS0";
        let result = update_kernel_param(
            current,
            "agent.https_proxy",
            "http://proxy:8080/path?query=1",
        );
        assert_eq!(
            result,
            "console=ttyS0 agent.https_proxy=http://proxy:8080/path?query=1"
        );

        // Test with no_proxy containing comma-separated list
        let current = "agent.log=debug";
        let result = update_kernel_param(current, "agent.no_proxy", "localhost,127.0.0.1,.local");
        assert_eq!(
            result,
            "agent.log=debug agent.no_proxy=localhost,127.0.0.1,.local"
        );
    }

    #[test]
    fn test_copy_artifacts_nonexistent_source() {
        let temp_dest = tempfile::tempdir().unwrap();
        let result = copy_artifacts("/nonexistent/source", temp_dest.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_hypervisor_name_empty() {
        let result = get_hypervisor_name("");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unknown shim"));
    }

    #[test]
    fn test_update_kernel_param_idempotent() {
        // Test that running update_kernel_param multiple times with same value is idempotent
        let initial = "console=ttyS0 agent.log=debug";

        // First update
        let result1 = update_kernel_param(initial, "agent.https_proxy", "http://proxy:8080");
        assert_eq!(
            result1,
            "console=ttyS0 agent.log=debug agent.https_proxy=http://proxy:8080"
        );

        // Second update with same value - should replace, not append
        let result2 = update_kernel_param(&result1, "agent.https_proxy", "http://proxy:8080");
        assert_eq!(
            result2,
            "console=ttyS0 agent.log=debug agent.https_proxy=http://proxy:8080"
        );

        // Verify no duplication occurred
        assert_eq!(result1, result2, "update_kernel_param must be idempotent");
    }

    #[test]
    fn test_update_kernel_param_multiple_runs_different_values() {
        // Test that updating with different values replaces correctly
        let initial = "console=ttyS0";

        let result1 = update_kernel_param(initial, "agent.https_proxy", "http://proxy1:8080");
        assert_eq!(
            result1,
            "console=ttyS0 agent.https_proxy=http://proxy1:8080"
        );

        let result2 = update_kernel_param(&result1, "agent.https_proxy", "http://proxy2:9090");
        assert_eq!(
            result2,
            "console=ttyS0 agent.https_proxy=http://proxy2:9090"
        );

        // Ensure only one proxy parameter exists
        assert_eq!(result2.matches("agent.https_proxy").count(), 1);
    }
}
