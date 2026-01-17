// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
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

    if Path::new(&config.host_install_dir).exists() {
        fs::remove_dir_all(&config.host_install_dir)?;
    }

    nfd::remove_nfd_rules(config).await?;

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

async fn configure_shim_config(config: &Config, shim: &str) -> Result<()> {
    let config_path = format!(
        "/host/{}",
        utils::get_kata_containers_config_path(shim, &config.dest_dir)
    );

    let kata_config_file = Path::new(&config_path).join(format!("configuration-{shim}.toml"));

    // The configuration file should exist after copy_artifacts() copied the kata artifacts.
    // If it doesn't exist, it means either the copy failed or this shim doesn't have a config
    // file in the artifacts (which would be a packaging error).
    if !kata_config_file.exists() {
        return Err(anyhow::anyhow!(
            "Configuration file not found: {kata_config_file:?}. This file should have been \
             copied from the kata-artifacts. Check that the shim '{}' has a valid configuration \
             file in the artifacts.",
            shim
        ));
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

    if shim.contains("tdx") {
        configure_tdx(config, shim, &kata_config_file).await?;
    }

    if config.dest_dir != "/opt/kata" {
        adjust_installation_prefix(config, shim, &kata_config_file).await?;
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

async fn configure_tdx(config: &Config, _shim: &str, config_file: &Path) -> Result<()> {
    let os_release_paths = ["/host/etc/os-release", "/host/usr/lib/os-release"];
    let mut os_release_content = String::new();
    for path in &os_release_paths {
        if Path::new(path).exists() {
            os_release_content = fs::read_to_string(path)?;
            break;
        }
    }

    let id = extract_os_release_field(&os_release_content, "ID");
    let version_id = extract_os_release_field(&os_release_content, "VERSION_ID");

    match (id.as_deref(), version_id.as_deref()) {
        (Some("ubuntu"), Some(v @ ("24.04" | "25.04" | "25.10"))) => {
            tdx_supported(config, "ubuntu", v, config_file).await?;
        }
        (Some("ubuntu"), Some(v)) => {
            log::warn!(
                "Distro ubuntu {} does not support TDX and the TDX related runtime classes will not work in your cluster!",
                v
            );
        }
        (Some("ubuntu"), None) => {
            log::warn!(
                "Distro ubuntu does not have VERSION_ID and the TDX related runtime classes will not work in your cluster!"
            );
        }
        (Some("centos"), Some("9")) => {
            tdx_supported(config, "centos", "9", config_file).await?;
        }
        (Some("centos"), Some(v)) => {
            log::warn!(
                "Distro centos {} does not support TDX and the TDX related runtime classes will not work in your cluster!",
                v
            );
        }
        (Some("centos"), None) => {
            log::warn!(
                "Distro centos does not have VERSION_ID and the TDX related runtime classes will not work in your cluster!"
            );
        }
        (Some(distro), _) => {
            log::warn!(
                "Distro {} does not support TDX and the TDX related runtime classes will not work in your cluster!",
                distro
            );
        }
        (None, _) => {
            log::warn!(
                "Could not determine OS distro and the TDX related runtime classes will not work in your cluster!"
            );
        }
    }

    Ok(())
}

fn extract_os_release_field(content: &str, field: &str) -> Option<String> {
    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if key == field {
                return Some(value.trim_matches('"').to_string());
            }
        }
    }
    None
}

async fn tdx_supported(
    _config: &Config,
    distro: &str,
    version: &str,
    config_file: &Path,
) -> Result<()> {
    let qemu_path = match distro {
        "ubuntu" => "/usr/bin/qemu-system-x86_64",
        "centos" => "/usr/libexec/qemu-kvm",
        _ => return Ok(()),
    };

    let ovmf_path = match distro {
        "ubuntu" => "/usr/share/ovmf/OVMF.fd",
        "centos" => "/usr/share/edk2/ovmf/OVMF.inteltdx.fd",
        _ => return Ok(()),
    };

    let current_qemu =
        toml_utils::get_toml_value(config_file, "hypervisor.qemu.path").unwrap_or_default();
    if current_qemu.contains("PLACEHOLDER_FOR_DISTRO_QEMU_WITH_TDX_SUPPORT") {
        log::debug!(
            "Updating hypervisor.qemu.path in {}: old=\"{}\" new=\"{}\"",
            config_file.display(),
            current_qemu,
            qemu_path
        );
        toml_utils::set_toml_value(
            config_file,
            "hypervisor.qemu.path",
            &format!("\"{qemu_path}\""),
        )?;
    }

    let current_ovmf =
        toml_utils::get_toml_value(config_file, "hypervisor.qemu.firmware").unwrap_or_default();
    if current_ovmf.contains("PLACEHOLDER_FOR_DISTRO_OVMF_WITH_TDX_SUPPORT") {
        log::debug!(
            "Updating hypervisor.qemu.firmware in {}: old=\"{}\" new=\"{}\"",
            config_file.display(),
            current_ovmf,
            ovmf_path
        );
        toml_utils::set_toml_value(
            config_file,
            "hypervisor.qemu.firmware",
            &format!("\"{ovmf_path}\""),
        )?;
    }

    let instructions = match distro {
        "ubuntu" => "https://github.com/canonical/tdx/tree/3.3",
        "centos" => "https://sigs.centos.org/virt/tdx",
        _ => "",
    };

    info!(
        "In order to use the tdx related runtime classes, ensure TDX is properly configured for {distro} {version} by following the instructions provided at: {instructions}"
    );

    Ok(())
}

async fn adjust_installation_prefix(config: &Config, shim: &str, config_file: &Path) -> Result<()> {
    let content = fs::read_to_string(config_file)?;

    if content.contains(&config.dest_dir) {
        return Ok(());
    }

    let new_content = content.replace("/opt/kata", &config.dest_dir);
    fs::write(config_file, new_content)?;

    if is_qemu_shim(shim) {
        adjust_qemu_cmdline(config, shim, config_file, None)?;
    }

    Ok(())
}

/// Note: The host_base_path parameter is kept to allow for unit testing with temporary directories
fn adjust_qemu_cmdline(
    config: &Config,
    shim: &str,
    config_file: &Path,
    host_base_path: Option<&str>,
) -> Result<()> {
    let qemu_share = match shim {
        "qemu-cca" => "qemu-cca-experimental".to_string(),
        "qemu-nvidia-gpu-snp" => "qemu-snp-experimental".to_string(),
        "qemu-nvidia-gpu-tdx" => "qemu-tdx-experimental".to_string(),
        s if is_qemu_shim(s) => "qemu".to_string(),
        _ => anyhow::bail!(
            "adjust_qemu_cmdline called with non-QEMU shim '{}'. This is a programming error.",
            shim
        ),
    };

    // Get QEMU path from config
    let qemu_binary = toml_utils::get_toml_value(config_file, ".hypervisor.qemu.path")?;
    let qemu_binary = qemu_binary.trim_matches('"');
    let qemu_binary_script = format!("{qemu_binary}-installation-prefix");

    // Use provided base path or default to /host for production
    let base_path = host_base_path.unwrap_or("/host");
    let qemu_binary_script_host_path = format!("{base_path}/{qemu_binary_script}");

    // Create wrapper script if it doesn't exist
    if !Path::new(&qemu_binary_script_host_path).exists() {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(&qemu_binary_script_host_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let script_content = format!(
            r#"#!/usr/bin/env bash

exec {} "$@" -L {}/share/kata-{}/qemu/
"#,
            qemu_binary, config.dest_dir, qemu_share
        );
        fs::write(&qemu_binary_script_host_path, script_content)?;
        let mut perms = fs::metadata(&qemu_binary_script_host_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&qemu_binary_script_host_path, perms)?;
    }

    // Update config file to use wrapper script using toml_edit
    let current_path =
        toml_utils::get_toml_value(config_file, "hypervisor.qemu.path").unwrap_or_default();
    if !current_path.contains(&qemu_binary_script) {
        log::debug!(
            "Updating hypervisor.qemu.path in {}: old=\"{}\" new=\"{}\"",
            config_file.display(),
            current_path,
            qemu_binary_script
        );
        toml_utils::set_toml_value(
            config_file,
            "hypervisor.qemu.path",
            &format!("\"{qemu_binary_script}\""),
        )?;
    }

    Ok(())
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

    /// Helper to create a minimal test Config with optional overrides
    fn create_test_config(shim: &str) -> crate::config::Config {
        crate::config::Config {
            node_name: "test-node".to_string(),
            debug: false,
            shims_for_arch: vec![shim.to_string()],
            default_shim_for_arch: shim.to_string(),
            allowed_hypervisor_annotations_for_arch: vec![],
            snapshotter_handler_mapping_for_arch: None,
            agent_https_proxy: None,
            agent_no_proxy: None,
            pull_type_mapping_for_arch: None,
            installation_prefix: None,
            multi_install_suffix: None,
            helm_post_delete_hook: false,
            experimental_setup_snapshotter: None,
            experimental_force_guest_pull_for_arch: vec![],
            dest_dir: "/opt/kata".to_string(),
            host_install_dir: "/host/opt/kata".to_string(),
            crio_drop_in_conf_dir: "/etc/crio/crio.conf.d/".to_string(),
            crio_drop_in_conf_file: "/etc/crio/crio.conf.d//99-kata-deploy".to_string(),
            crio_drop_in_conf_file_debug: "/etc/crio/crio.conf.d//100-debug".to_string(),
            containerd_conf_file: "/etc/containerd/config.toml".to_string(),
            containerd_conf_file_backup: "/etc/containerd/config.toml.bak".to_string(),
            containerd_drop_in_conf_file: "/opt/kata/containerd/config.d/kata-deploy.toml"
                .to_string(),
        }
    }

    #[test]
    fn test_extract_os_release_field() {
        let content = r#"ID=ubuntu
VERSION_ID="24.04"
"#;
        assert_eq!(extract_os_release_field(content, "ID"), Some("ubuntu".to_string()));
        assert_eq!(extract_os_release_field(content, "VERSION_ID"), Some("24.04".to_string()));
    }

    #[test]
    fn test_extract_os_release_field_empty() {
        let content = "";
        assert_eq!(extract_os_release_field(content, "ID"), None);
    }

    #[test]
    fn test_extract_os_release_field_missing() {
        let content = "ID=ubuntu\n";
        assert_eq!(extract_os_release_field(content, "VERSION_ID"), None);
    }

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
    fn test_tdx_qemu_path_ubuntu() {
        // Test TDX QEMU path resolution for Ubuntu
        let qemu_path = match "ubuntu" {
            "ubuntu" => "/usr/bin/qemu-system-x86_64",
            "centos" => "/usr/libexec/qemu-kvm",
            _ => "",
        };
        assert_eq!(qemu_path, "/usr/bin/qemu-system-x86_64");
    }

    #[test]
    fn test_tdx_qemu_path_centos() {
        // Test TDX QEMU path resolution for CentOS
        let qemu_path = match "centos" {
            "ubuntu" => "/usr/bin/qemu-system-x86_64",
            "centos" => "/usr/libexec/qemu-kvm",
            _ => "",
        };
        assert_eq!(qemu_path, "/usr/libexec/qemu-kvm");
    }

    #[test]
    fn test_tdx_ovmf_path_ubuntu() {
        // Test TDX OVMF path resolution for Ubuntu
        let ovmf_path = match "ubuntu" {
            "ubuntu" => "/usr/share/ovmf/OVMF.fd",
            "centos" => "/usr/share/edk2/ovmf/OVMF.inteltdx.fd",
            _ => "",
        };
        assert_eq!(ovmf_path, "/usr/share/ovmf/OVMF.fd");
    }

    #[test]
    fn test_tdx_ovmf_path_centos() {
        // Test TDX OVMF path resolution for CentOS
        let ovmf_path = match "centos" {
            "ubuntu" => "/usr/share/ovmf/OVMF.fd",
            "centos" => "/usr/share/edk2/ovmf/OVMF.inteltdx.fd",
            _ => "",
        };
        assert_eq!(ovmf_path, "/usr/share/edk2/ovmf/OVMF.inteltdx.fd");
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
    fn test_adjust_qemu_cmdline_qemu_standard() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir.path().join("configuration-qemu.toml");
        let host_base = temp_dir.path().join("host");

        // QEMU binary path (doesn't need to exist)
        let qemu_binary = "/opt/kata/bin/qemu-system-x86_64";

        // Create a minimal TOML config
        let toml_content = format!(
            r#"
[hypervisor.qemu]
path = "{}"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
"#,
            qemu_binary
        );
        std::fs::write(&config_file, toml_content).unwrap();

        let config = create_test_config("qemu");

        adjust_qemu_cmdline(
            &config,
            "qemu",
            &config_file,
            Some(host_base.to_str().unwrap()),
        )
        .unwrap();

        // Verify wrapper script was created
        let wrapper_script_path = host_base
            .join(qemu_binary.trim_start_matches('/'))
            .with_file_name(format!("qemu-system-x86_64-installation-prefix"));
        assert!(
            wrapper_script_path.exists(),
            "Wrapper script should be created at {:?}",
            wrapper_script_path
        );

        // Verify wrapper script content
        let content = std::fs::read_to_string(&wrapper_script_path).unwrap();
        assert!(content.contains("#!/usr/bin/env bash"));
        assert!(content.contains(&format!("exec {}", qemu_binary)));
        assert!(content.contains("-L /opt/kata/share/kata-qemu/qemu/"));

        // Verify TOML was updated
        let updated_path =
            crate::utils::toml::get_toml_value(&config_file, "hypervisor.qemu.path").unwrap();
        assert!(updated_path.contains("installation-prefix"));
    }

    #[test]
    fn test_adjust_qemu_cmdline_qemu_cca_experimental() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir.path().join("configuration-qemu-cca.toml");
        let host_base = temp_dir.path().join("host");

        let qemu_binary = "/opt/kata/bin/qemu-system-x86_64";

        let toml_content = format!(
            r#"
[hypervisor.qemu]
path = "{}"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
"#,
            qemu_binary
        );
        std::fs::write(&config_file, toml_content).unwrap();

        let config = create_test_config("qemu-cca");

        adjust_qemu_cmdline(
            &config,
            "qemu-cca",
            &config_file,
            Some(host_base.to_str().unwrap()),
        )
        .unwrap();

        // Verify wrapper script uses qemu-cca-experimental share path
        let wrapper_path = host_base
            .join(qemu_binary.trim_start_matches('/'))
            .with_file_name("qemu-system-x86_64-installation-prefix");
        let content = std::fs::read_to_string(&wrapper_path).unwrap();
        assert!(content.contains("-L /opt/kata/share/kata-qemu-cca-experimental/qemu/"));
    }

    #[test]
    fn test_adjust_qemu_cmdline_qemu_nvidia_gpu_snp() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir
            .path()
            .join("configuration-qemu-nvidia-gpu-snp.toml");
        let host_base = temp_dir.path().join("host");

        let qemu_binary = "/opt/kata/bin/qemu-system-x86_64";

        let toml_content = format!(
            r#"
[hypervisor.qemu]
path = "{}"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
"#,
            qemu_binary
        );
        std::fs::write(&config_file, toml_content).unwrap();

        let config = create_test_config("qemu-nvidia-gpu-snp");

        adjust_qemu_cmdline(
            &config,
            "qemu-nvidia-gpu-snp",
            &config_file,
            Some(host_base.to_str().unwrap()),
        )
        .unwrap();

        // Verify wrapper script uses qemu-snp-experimental share path
        let wrapper_path = host_base
            .join(qemu_binary.trim_start_matches('/'))
            .with_file_name("qemu-system-x86_64-installation-prefix");
        let content = std::fs::read_to_string(&wrapper_path).unwrap();
        assert!(content.contains("-L /opt/kata/share/kata-qemu-snp-experimental/qemu/"));
    }

    #[test]
    fn test_adjust_qemu_cmdline_qemu_nvidia_gpu_tdx() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir
            .path()
            .join("configuration-qemu-nvidia-gpu-tdx.toml");
        let host_base = temp_dir.path().join("host");

        let qemu_binary = "/opt/kata/bin/qemu-system-x86_64";

        let toml_content = format!(
            r#"
[hypervisor.qemu]
path = "{}"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
"#,
            qemu_binary
        );
        std::fs::write(&config_file, toml_content).unwrap();

        let config = create_test_config("qemu-nvidia-gpu-tdx");

        adjust_qemu_cmdline(
            &config,
            "qemu-nvidia-gpu-tdx",
            &config_file,
            Some(host_base.to_str().unwrap()),
        )
        .unwrap();

        // Verify wrapper script uses qemu-tdx-experimental share path
        let wrapper_path = host_base
            .join(qemu_binary.trim_start_matches('/'))
            .with_file_name("qemu-system-x86_64-installation-prefix");
        let content = std::fs::read_to_string(&wrapper_path).unwrap();
        assert!(content.contains("-L /opt/kata/share/kata-qemu-tdx-experimental/qemu/"));
    }

    #[test]
    fn test_adjust_qemu_cmdline_idempotent() {
        // Test that running adjust_qemu_cmdline multiple times is safe
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir.path().join("configuration-qemu.toml");
        let host_base = temp_dir.path().join("host");

        let qemu_binary = "/opt/kata/bin/qemu-system-x86_64";

        let toml_content = format!(
            r#"
[hypervisor.qemu]
path = "{}"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
"#,
            qemu_binary
        );
        std::fs::write(&config_file, toml_content).unwrap();

        let config = create_test_config("qemu");

        // Run twice - should be idempotent
        adjust_qemu_cmdline(
            &config,
            "qemu",
            &config_file,
            Some(host_base.to_str().unwrap()),
        )
        .unwrap();
        adjust_qemu_cmdline(
            &config,
            "qemu",
            &config_file,
            Some(host_base.to_str().unwrap()),
        )
        .unwrap();

        // Verify wrapper script exists and hasn't been corrupted
        let wrapper_path = host_base
            .join(qemu_binary.trim_start_matches('/'))
            .with_file_name("qemu-system-x86_64-installation-prefix");
        assert!(wrapper_path.exists());

        let content = std::fs::read_to_string(&wrapper_path).unwrap();
        // Count occurrences of shebang - should be exactly 1
        assert_eq!(content.matches("#!/usr/bin/env bash").count(), 1);
    }

    #[test]
    fn test_adjust_qemu_cmdline_invalid_shim() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_file = temp_dir.path().join("configuration-fc.toml");
        let host_base = temp_dir.path().join("host");

        let toml_content = r#"
[hypervisor.firecracker]
path = "/opt/kata/bin/firecracker"
"#;
        std::fs::write(&config_file, toml_content).unwrap();

        let config = create_test_config("fc");

        // Should fail for non-QEMU shim
        let result = adjust_qemu_cmdline(
            &config,
            "fc",
            &config_file,
            Some(host_base.to_str().unwrap()),
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("non-QEMU shim"));
    }

    #[test]
    fn test_adjust_qemu_cmdline_missing_config_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let host_base = temp_dir.path().join("host");
        let config_file = temp_dir.path().join("nonexistent.toml");

        let config = create_test_config("qemu");

        // Should fail for missing config file
        let result = adjust_qemu_cmdline(
            &config,
            "qemu",
            &config_file,
            Some(host_base.to_str().unwrap()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_adjust_qemu_cmdline_invalid_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let host_base = temp_dir.path().join("host");
        let config_file = temp_dir.path().join("invalid.toml");

        // Write invalid TOML
        std::fs::write(&config_file, "this is [ not valid { toml").unwrap();

        let config = create_test_config("qemu");

        // Should fail for invalid TOML
        let result = adjust_qemu_cmdline(
            &config,
            "qemu",
            &config_file,
            Some(host_base.to_str().unwrap()),
        );
        assert!(result.is_err());
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
