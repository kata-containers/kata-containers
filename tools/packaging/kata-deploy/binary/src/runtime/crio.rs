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

    utils::debug_log_file_contents(
        "CRI-O kata-deploy runtime drop-in",
        Path::new(&config.crio_drop_in_conf_file),
    );
    utils::debug_log_file_contents(
        "CRI-O kata-deploy debug drop-in",
        Path::new(&config.crio_drop_in_conf_file_debug),
    );

    Ok(())
}

/// Current on-disk content of the kata-deploy CRI-O drop-in file(s).
///
/// Returns `None` when the runtime drop-in does not exist yet. Callers use this
/// to tell whether re-applying the config actually changed anything, and
/// therefore whether a runtime restart is required. The debug drop-in is folded
/// in so a debug-flag change is also detected.
pub(crate) fn kata_cri_config_content(config: &Config) -> Option<String> {
    read_kata_crio_config(
        &config.crio_drop_in_conf_file,
        &config.crio_drop_in_conf_file_debug,
    )
}

/// Pure core of [`kata_cri_config_content`]: fold the runtime and debug drop-in
/// files into a single fingerprint. `None` iff the runtime drop-in is absent
/// (a missing debug drop-in is treated as empty so toggling debug is still
/// detected as a change).
fn read_kata_crio_config(runtime_file: &str, debug_file: &str) -> Option<String> {
    let runtime = fs::read_to_string(runtime_file).ok()?;
    let debug = fs::read_to_string(debug_file).unwrap_or_default();
    Some(format!("{runtime}\n{debug}"))
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
    use super::read_kata_crio_config;
    use std::fs;
    use tempfile::tempdir;

    // The CRI-config fingerprint underpins the job-mode "skip the
    // self-terminating restart when nothing changed" decision in
    // install_stage_cri: equal fingerprints across per-node Job retries mean the
    // runtime was already restarted with this config, so the retry can converge
    // instead of restarting (and getting killed) again.

    #[test]
    fn absent_runtime_drop_in_is_none() {
        let dir = tempdir().unwrap();
        let runtime = dir.path().join("99-kata-deploy");
        let debug = dir.path().join("100-debug");

        assert_eq!(
            read_kata_crio_config(runtime.to_str().unwrap(), debug.to_str().unwrap()),
            None,
            "no runtime drop-in on disk yet must read as None (fresh install -> restart)"
        );
    }

    #[test]
    fn present_runtime_absent_debug_folds_empty_debug() {
        let dir = tempdir().unwrap();
        let runtime = dir.path().join("99-kata-deploy");
        let debug = dir.path().join("100-debug");
        fs::write(&runtime, "runtime-config").unwrap();

        assert_eq!(
            read_kata_crio_config(runtime.to_str().unwrap(), debug.to_str().unwrap()),
            Some("runtime-config\n".to_string()),
        );
    }

    #[test]
    fn identical_config_is_stable_but_debug_change_is_detected() {
        let dir = tempdir().unwrap();
        let runtime = dir.path().join("99-kata-deploy");
        let debug = dir.path().join("100-debug");
        fs::write(&runtime, "runtime-config").unwrap();
        fs::write(&debug, "log_level = \"debug\"").unwrap();

        let before =
            read_kata_crio_config(runtime.to_str().unwrap(), debug.to_str().unwrap()).unwrap();

        // Re-reading the same bytes yields the same fingerprint: the retry sees
        // "unchanged" and takes the skip-restart path.
        let after_unchanged =
            read_kata_crio_config(runtime.to_str().unwrap(), debug.to_str().unwrap()).unwrap();
        assert_eq!(before, after_unchanged);

        // An upgrade that toggles debug must be observed as a change so the
        // runtime is restarted to pick it up.
        fs::write(&debug, "log_level = \"info\"").unwrap();
        let after_changed =
            read_kata_crio_config(runtime.to_str().unwrap(), debug.to_str().unwrap()).unwrap();
        assert_ne!(before, after_changed);
    }
}
