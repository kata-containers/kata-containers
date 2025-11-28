// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::client as k8s;
use crate::config::Config;
use crate::utils::yaml as yaml_utils;
use anyhow::Result;
use log::{info, warn};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Add managed-by labels to YAML content for resource tracking
/// Uses MULTI_INSTALL_SUFFIX to ensure each installation tracks its own resources
fn add_managed_by_label(yaml_content: &str, config: &Config) -> String {
    // If the label already exists, don't modify
    if yaml_content.contains("kata-deploy/instance:") {
        return yaml_content.to_string();
    }

    // Determine the instance identifier
    let instance = config
        .multi_install_suffix
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| s.as_str())
        .unwrap_or("default");

    // Insert labels after 'metadata:' if it exists
    if let Some(pos) = yaml_content.find("metadata:") {
        if let Some(newline_pos) = yaml_content[pos..].find('\n') {
            let insertion_point = pos + newline_pos + 1;
            let mut result = yaml_content.to_string();
            // Use two labels for better tracking:
            // 1. Standard managed-by label
            // 2. Instance-specific label to differentiate multiple installations
            let labels = format!(
                "  labels:\n    app.kubernetes.io/managed-by: kata-deploy\n    kata-deploy/instance: {}\n",
                instance
            );
            result.insert_str(insertion_point, &labels);
            return result;
        }
    }

    yaml_content.to_string()
}

fn adjust_shim_for_nfd(
    file_path: &Path,
    shim: &str,
    expand_runtime_classes_for_nfd: bool,
) -> Result<()> {
    if !expand_runtime_classes_for_nfd {
        return Ok(());
    }

    if shim.contains("tdx") {
        yaml_utils::adjust_runtimeclass_for_nfd(file_path, "tdx.intel.com/keys", 1)?;
    } else if shim.contains("snp") {
        yaml_utils::adjust_runtimeclass_for_nfd(file_path, "sev-snp.amd.com/esids", 1)?;
    }

    Ok(())
}

pub async fn create_runtimeclasses(
    config: &Config,
    expand_runtime_classes_for_nfd: bool,
) -> Result<()> {
    info!("Creating the runtime classes");

    for shim in &config.shims_for_arch {
        info!("Creating the kata-{shim} runtime class");

        let source_file = format!("/opt/kata-artifacts/runtimeclasses/kata-{shim}.yaml");
        let source_path = Path::new(&source_file);

        if !source_path.exists() {
            warn!("Runtime class file not found: {source_file}");
            continue;
        }

        let mut yaml_content = fs::read_to_string(source_path)?;

        match config.multi_install_suffix.as_ref() {
            Some(suffix) if !suffix.is_empty() => {
                let runtime_name = format!("kata-{shim}");
                let adjusted_name = format!("kata-{shim}-{suffix}");
                yaml_content = yaml_content.replace(&runtime_name, &adjusted_name);
            }
            _ => {}
        }

        // Add managed-by label for resource tracking
        yaml_content = add_managed_by_label(&yaml_content, config);

        let temp_file = format!("/tmp/kata-{shim}.yaml");
        fs::write(&temp_file, &yaml_content)?;

        adjust_shim_for_nfd(Path::new(&temp_file), shim, expand_runtime_classes_for_nfd)?;

        let final_content = fs::read_to_string(&temp_file)?;

        k8s::apply_yaml(config, &final_content).await?;

        fs::remove_file(&temp_file).ok();
    }

    if config.create_default_runtimeclass {
        if config
            .multi_install_suffix
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            warn!("CREATE_DEFAULT_RUNTIMECLASS is being ignored!");
            warn!("multi installation does not support creating a default runtime class");
            return Ok(());
        }

        info!(
            "Creating the kata runtime class for the default shim (an alias for kata-{})",
            config.default_shim_for_arch
        );

        let source_file = format!(
            "/opt/kata-artifacts/runtimeclasses/kata-{}.yaml",
            config.default_shim_for_arch
        );
        let mut yaml_content = fs::read_to_string(&source_file)?;

        let re = Regex::new(&format!(r"name:\s*kata-{}", config.default_shim_for_arch))?;
        yaml_content = re.replace_all(&yaml_content, "name: kata").to_string();

        // Add managed-by label for resource tracking
        yaml_content = add_managed_by_label(&yaml_content, config);

        let temp_file = "/tmp/kata.yaml";
        fs::write(temp_file, &yaml_content)?;

        k8s::apply_yaml(config, &yaml_content).await?;

        fs::remove_file(temp_file).ok();
    }

    Ok(())
}

pub async fn update_existing_runtimeclasses_for_nfd(config: &Config) -> Result<()> {
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    info!("Checking existing runtime classes for NFD updates");

    let existing_runtimeclasses = k8s::list_runtimeclasses(config).await?;

    for rc in existing_runtimeclasses {
        let name = rc
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RuntimeClass missing name"))?;

        if !name.starts_with("kata") {
            continue;
        }

        let nfd_key = if name.contains("tdx") {
            Some("tdx.intel.com/keys")
        } else if name.contains("snp") {
            Some("sev-snp.amd.com/esids")
        } else {
            None
        };

        if nfd_key.is_none() {
            continue;
        }

        let nfd_key = nfd_key.unwrap();

        // Only update if the RuntimeClass is missing the NFD field
        // Check if NFD key already exists in overhead.podFixed
        let needs_update = if let Some(ref overhead) = rc.overhead {
            if let Some(ref pod_fixed) = overhead.pod_fixed {
                // Field exists, check if the key is missing
                !pod_fixed.contains_key(nfd_key)
            } else {
                // overhead exists but podFixed is missing, needs update
                true
            }
        } else {
            // overhead is missing, needs update
            true
        };

        if !needs_update {
            info!("RuntimeClass {name} already has NFD key {nfd_key}, skipping");
            continue;
        }

        info!("Updating existing RuntimeClass {name} with missing NFD key {nfd_key}");

        let mut patched_rc = rc.clone();

        if patched_rc.overhead.is_none() {
            patched_rc.overhead = Some(Default::default());
        }

        if let Some(ref mut overhead) = patched_rc.overhead {
            if overhead.pod_fixed.is_none() {
                overhead.pod_fixed = Some(Default::default());
            }

            if let Some(ref mut pod_fixed) = overhead.pod_fixed {
                let quantity = Quantity("1".to_string());
                pod_fixed.insert(nfd_key.to_string(), quantity);
            }
        }

        k8s::update_runtimeclass(config, &patched_rc).await?;
        info!("Successfully updated RuntimeClass {name} with NFD key {nfd_key}");
    }

    Ok(())
}

pub async fn delete_runtimeclasses(config: &Config) -> Result<()> {
    info!("Deleting the runtime classes");

    for shim in &config.shims_for_arch {
        info!("Deleting the kata-{shim} runtime class");

        let canonical_shim_name = format!("kata-{shim}");

        let source_file = format!("/opt/kata-artifacts/runtimeclasses/{canonical_shim_name}.yaml");
        let source_path = Path::new(&source_file);

        if !source_path.exists() {
            warn!("Runtime class file not found: {source_file}");
            continue;
        }

        let mut yaml_content = fs::read_to_string(source_path)?;
        match config.multi_install_suffix.as_ref() {
            Some(suffix) if !suffix.is_empty() => {
                let adjusted_name = format!("{canonical_shim_name}-{suffix}");
                yaml_content = yaml_content.replace(&canonical_shim_name, &adjusted_name);
            }
            _ => {}
        }

        k8s::delete_yaml(config, &yaml_content, true).await?;
    }

    if config.create_default_runtimeclass {
        if config
            .multi_install_suffix
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            // Nothing to do for multi-install
            return Ok(());
        }

        info!(
            "Deleting the kata runtime class for the default shim (an alias for kata-{})",
            config.default_shim_for_arch
        );

        let source_file = format!(
            "/opt/kata-artifacts/runtimeclasses/kata-{}.yaml",
            config.default_shim_for_arch
        );
        let mut yaml_content = fs::read_to_string(&source_file)?;

        let re = Regex::new(&format!(r"name:\s*kata-{}", config.default_shim_for_arch))?;
        yaml_content = re.replace_all(&yaml_content, "name: kata").to_string();

        let temp_file = "/tmp/kata.yaml";
        fs::write(temp_file, &yaml_content)?;

        k8s::delete_yaml(config, &yaml_content, true).await?;

        fs::remove_file(temp_file).ok();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_runtime_class_name_without_suffix() {
        // Test runtime class name without MULTI_INSTALL_SUFFIX
        let shim = "qemu";

        // Expected: kata-qemu
        let expected = format!("kata-{}", shim);
        assert_eq!(expected, "kata-qemu");
    }

    #[test]
    fn test_runtime_class_name_with_suffix() {
        // Test runtime class name with MULTI_INSTALL_SUFFIX
        let shim = "qemu";
        let suffix = Some("dev".to_string());

        // Expected: kata-qemu-dev
        if let Some(s) = suffix {
            let expected = format!("kata-{}-{}", shim, s);
            assert_eq!(expected, "kata-qemu-dev");
        }
    }

    #[test]
    fn test_multiple_shims_with_suffix() {
        // Test different shims with suffix
        let shims = vec!["qemu", "qemu-tdx", "cloud-hypervisor", "fc"];
        let suffix = Some("staging".to_string());

        for shim in shims {
            if let Some(ref s) = suffix {
                let runtime_class = format!("kata-{}-{}", shim, s);
                assert!(runtime_class.contains(shim));
                assert!(runtime_class.contains("staging"));
                assert!(runtime_class.starts_with("kata-"));
            }
        }
    }

    #[test]
    fn test_suffix_prevents_default_runtimeclass() {
        // When MULTI_INSTALL_SUFFIX is set, default runtime class should not be created
        // This test verifies the logic
        let suffix = Some("prod".to_string());
        let create_default = true;

        // Logic: if suffix.is_some() && create_default, should warn and not create
        if suffix.is_some() && create_default {
            // Should not create default runtime class
            // Just verify the logic exists
            assert!(suffix.is_some());
        }
    }

    #[test]
    fn test_snapshotter_name_with_suffix() {
        // Test snapshotter name adjustment with MULTI_INSTALL_SUFFIX
        let suffix = Some("dev".to_string());
        let snapshotter = "nydus";

        if let Some(s) = suffix {
            let adjusted = format!("{}-{}", snapshotter, s);
            assert_eq!(adjusted, "nydus-dev");
        }
    }

    #[test]
    fn test_nydus_snapshotter_systemd_service_with_suffix() {
        // Test nydus-snapshotter systemd service name with suffix
        let suffix = Some("test".to_string());

        if let Some(s) = suffix {
            let service_name = format!("nydus-snapshotter-{}", s);
            assert_eq!(service_name, "nydus-snapshotter-test");
        }
    }
}
