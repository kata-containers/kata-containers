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

pub async fn install_artifacts(config: &Config, container_runtime: &str) -> Result<()> {
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
        configure_shim_config(config, shim, container_runtime).await?;
    }

    // Install custom runtime configuration files if enabled
    if config.custom_runtimes_enabled && !config.custom_runtimes.is_empty() {
        install_custom_runtime_configs(config, container_runtime)?;
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

/// Write the common drop-in configuration files for a shim.
/// This is shared between standard runtimes and custom runtimes.
fn write_common_drop_ins(
    config: &Config,
    shim: &str,
    config_d_dir: &str,
    container_runtime: &str,
) -> Result<()> {
    info!("Generating drop-in configuration files for shim: {}", shim);

    // 1. Installation prefix adjustments (if not default)
    if config.dest_dir != DEFAULT_KATA_INSTALL_DIR {
        info!("  - Installation prefix: {} (non-default)", config.dest_dir);
        let prefix_content = generate_installation_prefix_drop_in(config, shim)?;
        write_drop_in_file(config_d_dir, "10-installation-prefix.toml", &prefix_content)?;
    }

    // 2. Debug configuration (boolean flags only via drop-in)
    if config.debug {
        info!("  - Debug mode: enabled");
        let debug_content = generate_debug_drop_in(shim)?;
        write_drop_in_file(config_d_dir, "20-debug.toml", &debug_content)?;
    }

    // 2b. k0s: set kubelet root dir so ConfigMap/Secret volume propagation works (non-Rust shims only)
    if (container_runtime == "k0s-worker" || container_runtime == "k0s-controller")
        && !utils::is_rust_shim(shim)
    {
        info!("  - k0s: setting kubelet_root_dir for ConfigMap/Secret propagation");
        let k0s_content = generate_k0s_kubelet_root_drop_in();
        write_drop_in_file(config_d_dir, "22-k0s-kubelet-root.toml", &k0s_content)?;
    }

    // 3. Combined kernel_params (proxy, debug, etc.)
    // Reads base kernel_params from original config and combines with new params
    let kernel_params_content = generate_kernel_params_drop_in(config, shim)?;
    if !kernel_params_content.is_empty() {
        info!("  - Kernel parameters: configured");
        write_drop_in_file(config_d_dir, "30-kernel-params.toml", &kernel_params_content)?;
    }

    Ok(())
}

/// Each custom runtime gets an isolated directory under custom-runtimes/{handler}/
/// Custom runtimes inherit the same drop-in configurations as standard runtimes
/// (installation prefix, debug, kernel_params, and for k0s on Go/remote runtime: kubelet root) plus any user-provided overrides.
fn install_custom_runtime_configs(config: &Config, container_runtime: &str) -> Result<()> {
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
        let config_base =
            utils::get_kata_containers_original_config_path(&runtime.base_config, &config.dest_dir);
        let original_config = format!("/host{}/{}", config_base, base_config_filename);
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

            // Add warning comment to inform users about drop-in files
            add_kata_deploy_warning(Path::new(&dest_config))?;

            info!(
                "Copied config for custom runtime {}: {} -> {}",
                runtime.handler, original_config, dest_config
            );
        }

        // Generate the common drop-in files (shared with standard runtimes)
        write_common_drop_ins(config, &runtime.base_config, &config_d_dir, container_runtime)?;

        // Copy user-provided drop-in file if provided (at 50-overrides.toml)
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
///
/// Symlinks in the source tree are preserved at the destination (recreated as symlinks
/// instead of copying the target file). Absolute targets under the source root are
/// rewritten to the destination root so they remain valid.
fn copy_artifacts(src: &str, dst: &str) -> Result<()> {
    let src_path = Path::new(src);
    for entry in WalkDir::new(src).follow_links(false) {
        let entry = entry?;
        let src_path_entry = entry.path();
        let relative_path = src_path_entry.strip_prefix(src)?;
        let dst_path = Path::new(dst).join(relative_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dst_path)?;
        } else if entry.file_type().is_symlink() {
            // Preserve symlinks: create a symlink at destination instead of copying the target
            let link_target = fs::read_link(src_path_entry)
                .with_context(|| format!("Failed to read symlink: {:?}", src_path_entry))?;
            let new_target: std::path::PathBuf = if link_target.is_absolute() {
                // Rewrite absolute targets that point inside the source tree
                if let Ok(rel) = link_target.strip_prefix(src_path) {
                    Path::new(dst).join(rel)
                } else {
                    link_target.into()
                }
            } else {
                link_target.into()
            };

            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            match fs::remove_file(&dst_path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            std::os::unix::fs::symlink(&new_target, &dst_path)
                .with_context(|| format!("Failed to create symlink {:?} -> {:?}", dst_path, new_target))?;
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

            fs::copy(src_path_entry, &dst_path)?;
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

/// Warning comment to prepend to configuration files installed by kata-deploy.
/// This informs users to use drop-in files instead of modifying the base config directly.
const KATA_DEPLOY_CONFIG_WARNING: &str = r#"# =============================================================================
# IMPORTANT: This file is managed by kata-deploy. Do not modify it directly!
#
# To customize settings, create drop-in configuration files in the config.d/
# directory alongside this file. Drop-in files are processed in alphabetical
# order, with later files overriding earlier settings.
#
# Example: config.d/50-custom.toml
#
# See the kata-deploy documentation for more details:
#   https://github.com/kata-containers/kata-containers/tree/main/tools/packaging/kata-deploy
# =============================================================================

"#;

/// Prepends the kata-deploy warning comment to a configuration file.
/// This informs users to use drop-in files instead of modifying the base config.
fn add_kata_deploy_warning(config_file: &Path) -> Result<()> {
    let content = fs::read_to_string(config_file)
        .with_context(|| format!("Failed to read config file: {:?}", config_file))?;

    // Check if the warning is already present (idempotency)
    if content.contains("IMPORTANT: This file is managed by kata-deploy") {
        return Ok(());
    }

    let new_content = format!("{}{}", KATA_DEPLOY_CONFIG_WARNING, content);
    fs::write(config_file, new_content)
        .with_context(|| format!("Failed to write config file: {:?}", config_file))?;

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

    info!("Setting up runtime directory for shim: {}", shim);
    log::debug!("  Runtime config directory: {}", runtime_config_dir);

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

        // Add warning comment to inform users about drop-in files
        add_kata_deploy_warning(Path::new(&dest_config_file))?;

        info!("  Copied base config: {}", dest_config_file);
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

async fn configure_shim_config(config: &Config, shim: &str, container_runtime: &str) -> Result<()> {
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

    // Generate common drop-in files (shared with custom runtimes)
    write_common_drop_ins(config, shim, &config_d_dir, container_runtime)?;

    configure_hypervisor_annotations(config, shim, &kata_config_file).await?;

    if config
        .experimental_force_guest_pull_for_arch
        .contains(&shim.to_string())
    {
        configure_experimental_force_guest_pull(&kata_config_file).await?;
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

    info!("Created drop-in file: {}", drop_in_path);
    log::debug!("Drop-in file content:\n{}", content);
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

/// Generate drop-in content for k0s kubelet root directory.
/// k0s uses /var/lib/k0s/kubelet instead of /var/lib/kubelet; setting this
/// allows the runtime to match ConfigMap/Secret volume paths for propagation.
fn generate_k0s_kubelet_root_drop_in() -> String {
    r#"# k0s kubelet root directory
# Generated by kata-deploy for k0s (ConfigMap/Secret volume propagation)

[runtime]
kubelet_root_dir = "/var/lib/k0s/kubelet"
"#
    .to_string()
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

/// Get proxy value for a specific shim from config.
/// Handles both per-shim format ("qemu-tdx=http://proxy:8080;qemu-snp=http://proxy2:8080")
/// and global format ("http://proxy:8080").
fn get_proxy_value_for_shim(proxy_var: &Option<String>, shim: &str) -> Option<String> {
    match proxy_var {
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
        }
        Some(proxy) if !proxy.is_empty() => Some(proxy.clone()),
        _ => None,
    }
}

/// Read base kernel_params from the original configuration file.
fn read_base_kernel_params(config: &Config, shim: &str) -> Result<String> {
    let hypervisor_name = get_hypervisor_name(shim)?;
    let original_config_dir = format!(
        "/host{}",
        utils::get_kata_containers_original_config_path(shim, &config.dest_dir)
    );
    let original_config_file = format!("{}/configuration-{}.toml", original_config_dir, shim);
    let config_path = Path::new(&original_config_file);

    if !config_path.exists() {
        // If original config doesn't exist, return empty - this might happen in tests
        return Ok(String::new());
    }

    let kernel_params_path = format!("hypervisor.{}.kernel_params", hypervisor_name);
    let base_params = toml_utils::get_toml_value(config_path, &kernel_params_path)
        .unwrap_or_default();

    // Remove surrounding quotes if present
    Ok(base_params.trim_matches('"').to_string())
}

/// Generate drop-in content for all kernel_params modifications.
/// This reads the base kernel_params from the original config and combines
/// with proxy settings, debug settings, and any other kernel_params.
/// Using a single drop-in file avoids the TOML merge replacing behavior.
fn generate_kernel_params_drop_in(config: &Config, shim: &str) -> Result<String> {
    let mut additional_params = Vec::new();

    // Add proxy settings
    if let Some(proxy) = get_proxy_value_for_shim(&config.agent_https_proxy, shim) {
        additional_params.push(format!("agent.https_proxy={}", proxy));
    }
    if let Some(no_proxy) = get_proxy_value_for_shim(&config.agent_no_proxy, shim) {
        additional_params.push(format!("agent.no_proxy={}", no_proxy));
    }

    // Add debug settings
    if config.debug {
        additional_params.push("agent.log=debug".to_string());
        additional_params.push("initcall_debug".to_string());
    }

    // If no additional params to set, return empty (base params are in original config)
    if additional_params.is_empty() {
        return Ok(String::new());
    }

    // Read base kernel_params from original config
    let base_params = read_base_kernel_params(config, shim)?;

    // Combine base params with additional params
    let combined_params = if base_params.is_empty() {
        additional_params.join(" ")
    } else {
        format!("{} {}", base_params, additional_params.join(" "))
    };

    let hypervisor_name = get_hypervisor_name(shim)?;

    let content = format!(
        r#"# Kernel parameters
# Generated by kata-deploy
# This file combines base kernel_params with additional settings

[hypervisor.{}]
kernel_params = "{}"
"#,
        hypervisor_name, combined_params
    );

    Ok(content)
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
    use rstest::rstest;

    #[rstest]
    #[case("qemu", "qemu")]
    #[case("qemu-tdx", "qemu")]
    #[case("qemu-snp", "qemu")]
    #[case("qemu-se", "qemu")]
    #[case("qemu-coco-dev", "qemu")]
    #[case("qemu-cca", "qemu")]
    #[case("qemu-nvidia-gpu", "qemu")]
    #[case("qemu-nvidia-gpu-tdx", "qemu")]
    #[case("qemu-nvidia-gpu-snp", "qemu")]
    #[case("qemu-runtime-rs", "qemu")]
    #[case("qemu-coco-dev-runtime-rs", "qemu")]
    #[case("qemu-se-runtime-rs", "qemu")]
    #[case("qemu-snp-runtime-rs", "qemu")]
    #[case("qemu-tdx-runtime-rs", "qemu")]
    fn test_get_hypervisor_name_qemu_variants(#[case] shim: &str, #[case] expected: &str) {
        assert_eq!(get_hypervisor_name(shim).unwrap(), expected);
    }

    #[rstest]
    #[case("clh", "clh")]
    #[case("cloud-hypervisor", "cloud-hypervisor")]
    #[case("dragonball", "dragonball")]
    #[case("fc", "firecracker")]
    #[case("firecracker", "firecracker")]
    #[case("remote", "remote")]
    fn test_get_hypervisor_name_other_hypervisors(#[case] shim: &str, #[case] expected: &str) {
        assert_eq!(get_hypervisor_name(shim).unwrap(), expected);
    }

    #[rstest]
    #[case("")]
    #[case("unknown-shim")]
    #[case("custom")]
    fn test_get_hypervisor_name_unknown(#[case] shim: &str) {
        let result = get_hypervisor_name(shim);
        assert!(result.is_err(), "Unknown shim should return an error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains(&format!("Unknown shim '{}'", shim)),
            "Error message should mention the unknown shim"
        );
        assert!(
            err_msg.contains("Valid shims are:"),
            "Error message should list valid shims"
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
    fn test_copy_artifacts_nonexistent_source() {
        let temp_dest = tempfile::tempdir().unwrap();
        let result = copy_artifacts("/nonexistent/source", temp_dest.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_copy_artifacts_preserves_symlinks() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();

        // Create a real file and a symlink pointing to it
        let real_file = src_dir.path().join("real-file.txt");
        fs::write(&real_file, "actual content").unwrap();
        let link_path = src_dir.path().join("link-to-real");
        std::os::unix::fs::symlink(&real_file, &link_path).unwrap();

        copy_artifacts(
            src_dir.path().to_str().unwrap(),
            dst_dir.path().to_str().unwrap(),
        )
        .unwrap();

        let dst_link = dst_dir.path().join("link-to-real");
        let dst_real = dst_dir.path().join("real-file.txt");
        assert!(dst_real.exists(), "real file should be copied");
        assert!(dst_link.is_symlink(), "destination should be a symlink");
        assert_eq!(
            fs::read_link(&dst_link).unwrap(),
            dst_real,
            "symlink should point to the real file in the same tree"
        );
        assert_eq!(
            fs::read_to_string(&dst_link).unwrap(),
            "actual content",
            "following the symlink should yield the real content"
        );
    }

}
