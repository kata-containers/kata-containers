// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::utils;
use anyhow::Result;
use log::info;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Parameters for configuring a CRI-O runtime
struct CrioRuntimeConfig<'a> {
    runtime_name: &'a str,
    runtime_path: &'a str,
    config_path: &'a str,
    guest_pull: bool,
}

/// Write CRI-O runtime configuration to file
fn write_crio_runtime_config(file: &mut fs::File, runtime_config: &CrioRuntimeConfig) -> Result<()> {
    let kata_conf = format!("crio.runtime.runtimes.{}", runtime_config.runtime_name);

    writeln!(file)?;
    writeln!(file, "[{kata_conf}]")?;
    writeln!(
        file,
        r#"	runtime_path = "{}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	runtime_config_path = "{}"
	privileged_without_host_devices = true"#,
        runtime_config.runtime_path,
        runtime_config.config_path
    )?;

    if runtime_config.guest_pull {
        writeln!(file, r#"	runtime_pull_image = true"#)?;
    }

    Ok(())
}

pub async fn configure_crio_runtime(config: &Config, shim: &str) -> Result<()> {
    let adjusted_shim = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{shim}-{suffix}"),
        _ => shim.to_string(),
    };

    let runtime_name = format!("kata-{adjusted_shim}");
    let runtime_path = utils::get_kata_containers_runtime_path(shim, &config.dest_dir);
    let config_path = format!(
        "{}/configuration-{shim}.toml",
        utils::get_kata_containers_config_path(shim, &config.dest_dir)
    );

    // Determine if guest-pull is enabled from mapping
    let guest_pull = config.pull_type_mapping_for_arch.as_ref().map_or(false, |mapping| {
        mapping.split(',').any(|m| {
            let parts: Vec<&str> = m.split(':').collect();
            parts.len() == 2 && parts[0] == shim && parts[1] == "guest-pull"
        })
    });

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.crio_drop_in_conf_file)?;

    write_crio_runtime_config(
        &mut file,
        &CrioRuntimeConfig {
            runtime_name: &runtime_name,
            runtime_path: &runtime_path,
            config_path: &config_path,
            guest_pull,
        },
    )?;

    Ok(())
}

pub async fn configure_crio(config: &Config) -> Result<()> {
    info!("Add Kata Containers as a supported runtime for CRIO:");

    fs::create_dir_all(&config.crio_drop_in_conf_dir)?;

    if Path::new(&config.crio_drop_in_conf_file).exists() {
        fs::remove_file(&config.crio_drop_in_conf_file)?;
    }
    fs::File::create(&config.crio_drop_in_conf_file)?;

    if Path::new(&config.crio_drop_in_conf_file_debug).exists() {
        fs::remove_file(&config.crio_drop_in_conf_file_debug)?;
    }
    fs::File::create(&config.crio_drop_in_conf_file_debug)?;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.crio_drop_in_conf_file)?;
    writeln!(
        file,
        r#"[crio]
  storage_option = [
	"overlay.skip_mount_home=true",
  ]"#
    )?;

    for shim in &config.shims_for_arch {
        configure_crio_runtime(config, shim).await?;
    }

    if config.custom_runtimes_enabled && !config.custom_runtimes.is_empty() {
        info!(
            "Configuring {} custom runtime(s) for CRI-O",
            config.custom_runtimes.len()
        );
        for custom_runtime in &config.custom_runtimes {
            configure_custom_crio_runtime(config, custom_runtime).await?;
        }
    }

    if config.debug {
        let mut debug_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.crio_drop_in_conf_file_debug)?;
        writeln!(
            debug_file,
            r#"[crio.runtime]
log_level = "debug""#
        )?;
    }

    Ok(())
}

pub async fn configure_custom_crio_runtime(
    config: &Config,
    custom_runtime: &crate::config::CustomRuntime,
) -> Result<()> {
    info!("Configuring custom CRI-O runtime: {}", custom_runtime.handler);

    // Derive shim path from base_config (uses existing is_rust_shim logic)
    let runtime_path = utils::get_kata_containers_runtime_path(&custom_runtime.base_config, &config.dest_dir);

    // Config path points to the isolated custom runtime directory
    let config_path = format!(
        "{}/share/defaults/kata-containers/custom-runtimes/{}/configuration-{}.toml",
        config.dest_dir,
        custom_runtime.handler,
        custom_runtime.base_config
    );

    let guest_pull = custom_runtime
        .crio_pull_type
        .as_ref()
        .map_or(false, |pt| pt == "guest-pull");

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.crio_drop_in_conf_file)?;

    write_crio_runtime_config(
        &mut file,
        &CrioRuntimeConfig {
            runtime_name: &custom_runtime.handler,
            runtime_path: &runtime_path,
            config_path: &config_path,
            guest_pull,
        },
    )?;

    Ok(())
}

pub async fn cleanup_crio(config: &Config) -> Result<()> {
    if Path::new(&config.crio_drop_in_conf_file).exists() {
        fs::remove_file(&config.crio_drop_in_conf_file)?;
    }

    if config.debug && Path::new(&config.crio_drop_in_conf_file_debug).exists() {
        fs::remove_file(&config.crio_drop_in_conf_file_debug)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::CustomRuntime;

    #[test]
    fn test_custom_runtime_pull_type_guest_pull() {
        let runtime = CustomRuntime {
            handler: "kata-my-coco".to_string(),
            base_config: "qemu-nvidia-gpu".to_string(),
            drop_in_file: None,
            containerd_snapshotter: None,
            crio_pull_type: Some("guest-pull".to_string()),
        };

        assert_eq!(runtime.crio_pull_type, Some("guest-pull".to_string()));
        assert_eq!(runtime.base_config, "qemu-nvidia-gpu");
    }

    #[test]
    fn test_custom_runtime_pull_type_none() {
        let runtime = CustomRuntime {
            handler: "kata-basic".to_string(),
            base_config: "qemu".to_string(),
            drop_in_file: None,
            containerd_snapshotter: None,
            crio_pull_type: None,
        };

        assert!(runtime.crio_pull_type.is_none());
    }

    #[test]
    fn test_custom_runtime_pull_type_check_logic() {
        // Case 1: guest-pull should apply
        let pull_type1: Option<String> = Some("guest-pull".to_string());
        let should_apply1 = matches!(pull_type1.as_deref(), Some("guest-pull"));
        assert!(should_apply1);

        // Case 2: None should not apply
        let pull_type2: Option<String> = None;
        let should_apply2 = matches!(pull_type2.as_deref(), Some("guest-pull"));
        assert!(!should_apply2);

        // Case 3: Other values should not apply
        let pull_type3: Option<String> = Some("default".to_string());
        let should_apply3 = matches!(pull_type3.as_deref(), Some("guest-pull"));
        assert!(!should_apply3);
    }

    #[test]
    fn test_custom_runtime_crio_config_path_format() {
        // Custom runtimes use isolated directories: custom-runtimes/{handler}/configuration-{base}.toml
        let handler = "kata-my-runtime";
        let base_config = "qemu-nvidia-gpu";
        let dest_dir = "/opt/kata";
        let config_path = format!(
            "{}/share/defaults/kata-containers/custom-runtimes/{}/configuration-{}.toml",
            dest_dir, handler, base_config
        );

        assert_eq!(
            config_path,
            "/opt/kata/share/defaults/kata-containers/custom-runtimes/kata-my-runtime/configuration-qemu-nvidia-gpu.toml"
        );
    }
}
