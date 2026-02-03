// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::Result;
use log::info;
use std::fs;
use std::path::Path;

pub async fn configure_erofs_snapshotter(
    _config: &Config,
    configuration_file: &Path,
) -> Result<()> {
    info!("Configuring erofs-snapshotter");

    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.cri.v1.images\".discard_unpacked_layers",
        "false",
    )?;

    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.service.v1.diff-service\".default",
        "[\"erofs\",\"walking\"]",
    )?;

    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.snapshotter.v1.erofs\".enable_fsverity",
        "true",
    )?;
    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.snapshotter.v1.erofs\".set_immutable",
        "true",
    )?;

    Ok(())
}

pub async fn configure_nydus_snapshotter(
    config: &Config,
    configuration_file: &Path,
    pluginid: &str,
) -> Result<()> {
    info!("Configuring nydus-snapshotter");

    let nydus = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-{suffix}"),
        _ => "nydus".to_string(),
    };

    let containerd_nydus = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-snapshotter-{suffix}"),
        _ => "nydus-snapshotter".to_string(),
    };

    toml_utils::set_toml_value(
        configuration_file,
        &format!(".plugins.{pluginid}.disable_snapshot_annotations"),
        "false",
    )?;

    toml_utils::set_toml_value(
        configuration_file,
        &format!(".proxy_plugins.\"{nydus}\".type"),
        "\"snapshot\"",
    )?;
    toml_utils::set_toml_value(
        configuration_file,
        &format!(".proxy_plugins.\"{nydus}\".address"),
        &format!("\"/run/{containerd_nydus}/containerd-nydus-grpc.sock\""),
    )?;

    Ok(())
}

pub async fn configure_snapshotter(
    snapshotter: &str,
    runtime: &str,
    config: &Config,
) -> Result<()> {
    // Get all paths and drop-in capability in one call
    let paths = config.get_containerd_paths(runtime).await?;
    
    // Read containerd version from config_file to determine pluginid
    let pluginid = if fs::read_to_string(&paths.config_file)
        .unwrap_or_default()
        .contains("version = 3")
    {
        "\"io.containerd.cri.v1.images\""
    } else {
        "\"io.containerd.grpc.v1.cri\".containerd"
    };

    let configuration_file: std::path::PathBuf = if paths.use_drop_in {
        // Only add /host prefix if path is not in /etc/containerd (which is mounted from host)
        let base_path = if paths.drop_in_file.starts_with("/etc/containerd/") {
            Path::new(&paths.drop_in_file).to_path_buf()
        } else {
            // Need to add /host prefix for paths outside /etc/containerd
            let drop_in_path = paths.drop_in_file.trim_start_matches('/');
            Path::new("/host").join(drop_in_path)
        };

        log::debug!("Snapshotter using drop-in config file: {:?}", base_path);
        base_path
    } else {
        log::debug!("Snapshotter using main config file: {}", paths.config_file);
        Path::new(&paths.config_file).to_path_buf()
    };

    match snapshotter {
        "nydus" => {
            configure_nydus_snapshotter(config, &configuration_file, pluginid).await?;

            let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
                Some(suffix) if !suffix.is_empty() => format!("nydus-snapshotter-{suffix}"),
                _ => "nydus-snapshotter".to_string(),
            };

            utils::host_systemctl(&["restart", &nydus_snapshotter])?;
        }
        "erofs" => {
            configure_erofs_snapshotter(config, &configuration_file).await?;
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported snapshotter: {snapshotter}"));
        }
    }

    Ok(())
}

pub async fn install_nydus_snapshotter(config: &Config) -> Result<()> {
    info!("Deploying nydus-snapshotter");

    let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-snapshotter-{suffix}"),
        _ => "nydus-snapshotter".to_string(),
    };

    // Clean up existing nydus-snapshotter state to ensure fresh start with new version.
    // This is safe across all K8s distributions (k3s, rke2, k0s, microk8s, etc.) because
    // we only touch the nydus data directory, not containerd's internals.
    // When containerd tries to use non-existent snapshots, it will re-pull/re-unpack.
    let nydus_data_dir = format!("/host/var/lib/{nydus_snapshotter}");
    info!("Cleaning up existing nydus-snapshotter state at {}", nydus_data_dir);
    
    // Stop the service first if it exists (ignore errors if not running)
    let _ = utils::host_systemctl(&["stop", &format!("{nydus_snapshotter}.service")]);
    
    // Remove the data directory to clean up old snapshots with potentially incorrect labels
    if Path::new(&nydus_data_dir).exists() {
        info!("Removing nydus data directory: {}", nydus_data_dir);
        fs::remove_dir_all(&nydus_data_dir).ok();
    }

    let config_guest_pulling = "/opt/kata-artifacts/nydus-snapshotter/config-guest-pulling.toml";
    let nydus_snapshotter_service =
        "/opt/kata-artifacts/nydus-snapshotter/nydus-snapshotter.service";

    let mut config_content = fs::read_to_string(config_guest_pulling)?;
    config_content = config_content.replace(
        "@SNAPSHOTTER_ROOT_DIR@",
        &format!("/var/lib/{nydus_snapshotter}"),
    );
    config_content = config_content.replace(
        "@SNAPSHOTTER_GRPC_SOCKET_ADDRESS@",
        &format!("/run/{nydus_snapshotter}/containerd-nydus-grpc.sock"),
    );
    config_content = config_content.replace(
        "@NYDUS_OVERLAYFS_PATH@",
        &format!(
            "{}/nydus-snapshotter/nydus-overlayfs",
            &config
                .host_install_dir
                .strip_prefix("/host")
                .unwrap_or(&config.host_install_dir)
        ),
    );

    let mut service_content = fs::read_to_string(nydus_snapshotter_service)?;
    service_content = service_content.replace(
        "@CONTAINERD_NYDUS_GRPC_BINARY@",
        &format!(
            "{}/nydus-snapshotter/containerd-nydus-grpc",
            &config
                .host_install_dir
                .strip_prefix("/host")
                .unwrap_or(&config.host_install_dir)
        ),
    );
    service_content = service_content.replace(
        "@CONFIG_GUEST_PULLING@",
        &format!(
            "{}/nydus-snapshotter/config-guest-pulling.toml",
            &config
                .host_install_dir
                .strip_prefix("/host")
                .unwrap_or(&config.host_install_dir)
        ),
    );

    fs::create_dir_all(format!("{}/nydus-snapshotter", config.host_install_dir))?;

    fs::copy(
        "/opt/kata-artifacts/nydus-snapshotter/containerd-nydus-grpc",
        format!(
            "{}/nydus-snapshotter/containerd-nydus-grpc",
            config.host_install_dir
        ),
    )?;
    fs::copy(
        "/opt/kata-artifacts/nydus-snapshotter/nydus-overlayfs",
        format!(
            "{}/nydus-snapshotter/nydus-overlayfs",
            config.host_install_dir
        ),
    )?;

    fs::write(
        format!(
            "{}/nydus-snapshotter/config-guest-pulling.toml",
            config.host_install_dir
        ),
        config_content,
    )?;

    fs::write(
        format!("/host/etc/systemd/system/{nydus_snapshotter}.service"),
        service_content,
    )?;

    utils::host_systemctl(&["daemon-reload"])?;
    utils::host_systemctl(&["enable", &format!("{nydus_snapshotter}.service")])?;

    Ok(())
}

pub async fn uninstall_nydus_snapshotter(config: &Config) -> Result<()> {
    info!("Removing deployed nydus-snapshotter");

    let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-snapshotter-{suffix}"),
        _ => "nydus-snapshotter".to_string(),
    };

    utils::host_systemctl(&["disable", "--now", &format!("{nydus_snapshotter}.service")])?;

    fs::remove_file(format!(
        "/host/etc/systemd/system/{nydus_snapshotter}.service"
    ))
    .ok();
    fs::remove_dir_all(format!("{}/nydus-snapshotter", config.host_install_dir)).ok();

    utils::host_systemctl(&["daemon-reload"])?;

    Ok(())
}

pub async fn install_snapshotter(snapshotter: &str, config: &Config) -> Result<()> {
    match snapshotter {
        "erofs" => {
            // erofs is a containerd built-in snapshotter, no installation needed
        }
        "nydus" => {
            install_nydus_snapshotter(config).await?;
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported snapshotter: {snapshotter}"));
        }
    }

    Ok(())
}

pub async fn uninstall_snapshotter(snapshotter: &str, config: &Config) -> Result<()> {
    match snapshotter {
        "nydus" => {
            uninstall_nydus_snapshotter(config).await?;
        }
        _ => {
            // No cleanup needed for erofs
        }
    }

    Ok(())
}
