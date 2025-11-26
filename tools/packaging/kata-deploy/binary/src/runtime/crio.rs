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

pub async fn configure_crio_runtime(config: &Config, shim: &str) -> Result<()> {
    let adjusted_shim = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{shim}-{suffix}"),
        _ => shim.to_string(),
    };
    let runtime = format!("kata-{adjusted_shim}");
    let configuration = format!("configuration-{shim}");

    let config_path = utils::get_kata_containers_config_path(shim, &config.dest_dir);
    let kata_path = utils::get_kata_containers_runtime_path(shim, &config.dest_dir);
    let kata_conf = format!("crio.runtime.runtimes.{runtime}");
    let kata_config_path = format!("{config_path}/{configuration}.toml");

    let conf_file = Path::new(&config.crio_drop_in_conf_file);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(conf_file)?;

    writeln!(file)?;
    writeln!(file, "[{kata_conf}]")?;
    writeln!(
        file,
        r#"	runtime_path = "{}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	runtime_config_path = "{}"
	privileged_without_host_devices = true"#,
        kata_path,
        kata_config_path
    )?;

    match config.pull_type_mapping_for_arch.as_ref() {
        Some(mapping) => {
            let pull_types: Vec<&str> = mapping.split(',').collect();
            for m in pull_types {
                let parts: Vec<&str> = m.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                let key = parts[0];
                let value = parts[1];

                if key != shim || value == "default" {
                    continue;
                }

                match value {
                    "guest-pull" => writeln!(file, r#"	runtime_pull_image = true"#)?,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unsupported pull type '{value}' for {shim}"
                        ))
                    }
                }
                break;
            }
        }
        _ => {}
    }

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
