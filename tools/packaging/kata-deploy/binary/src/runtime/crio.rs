// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Config, CustomRuntime};
use crate::utils;
use anyhow::Result;
use log::info;
use std::fs;
use std::io::Write;
use std::path::Path;

struct CrioRuntimeParams<'a> {
    /// Runtime name (e.g., "kata-qemu")
    runtime_name: &'a str,
    /// Path to the shim binary
    runtime_path: String,
    /// Path to the kata configuration file
    config_path: String,
    /// Whether to enable guest-pull (runtime_pull_image = true)
    guest_pull: bool,
}

fn write_crio_runtime_config(file: &mut fs::File, params: &CrioRuntimeParams) -> Result<()> {
    let kata_conf = format!("crio.runtime.runtimes.{}", params.runtime_name);

    writeln!(file)?;
    writeln!(file, "[{kata_conf}]")?;
    writeln!(
        file,
        r#"	runtime_path = "{}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	runtime_config_path = "{}"
	privileged_without_host_devices = true"#,
        params.runtime_path, params.config_path
    )?;

    if params.guest_pull {
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
    let configuration = format!("configuration-{shim}");

    // Determine if guest-pull is configured for this shim
    let guest_pull = config
        .pull_type_mapping_for_arch
        .as_ref()
        .map(|mapping| {
            mapping.split(',').any(|m| {
                let parts: Vec<&str> = m.split(':').collect();
                parts.len() == 2 && parts[0] == shim && parts[1] == "guest-pull"
            })
        })
        .unwrap_or(false);

    let params = CrioRuntimeParams {
        runtime_name: &runtime_name,
        runtime_path: utils::get_kata_containers_runtime_path(shim, &config.dest_dir),
        config_path: format!(
            "{}/{}.toml",
            utils::get_kata_containers_config_path(shim, &config.dest_dir),
            configuration
        ),
        guest_pull,
    };

    let conf_file = Path::new(&config.crio_drop_in_conf_file);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(conf_file)?;

    write_crio_runtime_config(&mut file, &params)
}

pub async fn configure_custom_crio_runtime(
    config: &Config,
    custom_runtime: &CustomRuntime,
) -> Result<()> {
    info!(
        "Configuring custom CRI-O runtime: {}",
        custom_runtime.handler
    );

    let guest_pull = custom_runtime
        .crio_pull_type
        .as_ref()
        .map(|p| p == "guest-pull")
        .unwrap_or(false);

    let params = CrioRuntimeParams {
        runtime_name: &custom_runtime.handler,
        runtime_path: utils::get_kata_containers_runtime_path(
            &custom_runtime.base_config,
            &config.dest_dir,
        ),
        config_path: format!(
            "{}/share/defaults/kata-containers/custom-runtimes/{}/configuration-{}.toml",
            config.dest_dir, custom_runtime.handler, custom_runtime.base_config
        ),
        guest_pull,
    };

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.crio_drop_in_conf_file)?;

    write_crio_runtime_config(&mut file, &params)
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

    if config.custom_runtimes_enabled {
        if config.custom_runtimes.is_empty() {
            anyhow::bail!(
                "Custom runtimes enabled but no custom runtimes found in configuration. \
                 Check that custom-runtimes.list exists and is readable."
            );
        }
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

pub async fn cleanup_crio(config: &Config) -> Result<()> {
    if Path::new(&config.crio_drop_in_conf_file).exists() {
        fs::remove_file(&config.crio_drop_in_conf_file)?;
    }

    if config.debug && Path::new(&config.crio_drop_in_conf_file_debug).exists() {
        fs::remove_file(&config.crio_drop_in_conf_file_debug)?;
    }

    Ok(())
}
