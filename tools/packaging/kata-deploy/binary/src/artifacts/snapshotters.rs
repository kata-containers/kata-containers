// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::runtime::containerd;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::Result;
use log::{info, warn};
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

    // Runtime plugin id (from paths or by reading config), then map to table where disable_snapshot_annotations lives.
    let runtime_plugin_id = match &paths.plugin_id {
        Some(id) => id.as_str(),
        None => containerd::get_containerd_pluginid(&paths.config_file)?,
    };
    let pluginid = containerd::pluginid_for_snapshotter_annotations(runtime_plugin_id, &paths.config_file)?;

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

/// Clean up all nydus-related entries from containerd's metadata store across all namespaces.
///
/// ## Why this must run before stopping the nydus service
///
/// `ctr snapshots rm` goes through containerd's metadata layer which calls the nydus gRPC
/// backend to physically remove the snapshot. If the service is stopped first, the backend
/// call fails and the BoltDB record is left behind as a stale entry.
///
/// Stale snapshot records in BoltDB cause subsequent image pulls to fail with:
///   "unable to prepare extraction snapshot: target snapshot sha256:...: already exists"
///
/// The failure path: containerd's metadata `Prepare` finds the target chainID in BoltDB and
/// returns AlreadyExists without calling the backend. The unpacker then calls `Stat`, which
/// finds the BoltDB record, but delegates to the backend which returns NotFound (nydus data
/// wiped). The unpacker treats this as a transient race and retries 3 times; all 3 fail the
/// same way, and the pull is aborted.
///
/// ## What we clean
///
/// The containerd BoltDB schema has these nydus-relevant buckets per namespace:
///   - `snapshots/nydus/*`   — 100% nydus-specific; MUST be cleaned (triggers the pull bug)
///   - `containers/*`        — records carry `snapshotter=nydus` + `snapshotKey`; after
///                             removing the snapshots these become dangling references.
///                             In a normal CI run they are already gone, but an aborted run
///                             can leave orphaned container records that confuse reconciliation.
///   - `images/*`            — snapshotter-agnostic (just manifest digest + labels); leave
///                             them so the next pull can skip re-downloading content.
///   - `content/blob/*`      — shared across all snapshotters; must NOT be removed.
///   - `leases/*`, `ingests/*` — temporary; expire and are GC'd automatically.
///
/// Note: containerd's garbage collector will NOT remove stale snapshots for us, because the
/// image record (a GC root) still references the content blobs which reference the snapshots
/// via gc.ref labels, keeping the entire chain alive in the GC graph.
///
/// ## Snapshot removal ordering
///
/// Snapshots have parent→child relationships; a parent cannot be removed while children
/// exist. The retry loop removes whatever it can each round (leaves first), then retries
/// until nothing remains or no progress is made.
///
/// ## Return value
///
/// Returns `true` only if ALL snapshots were removed from ALL namespaces.  A `false` return
/// means at least one snapshot could not be removed — almost certainly because a workload is
/// still actively using it.  Callers MUST NOT wipe the nydus data directory in that case:
/// doing so would corrupt running containers whose rootfs mounts still depend on that data.
fn cleanup_containerd_nydus_snapshots(containerd_snapshotter: &str) -> bool {
    info!(
        "Cleaning up nydus entries from containerd metadata (snapshotter: '{containerd_snapshotter}')"
    );

    // Discover all containerd namespaces so every namespace is cleaned, not just k8s.io.
    let namespaces = match utils::host_exec(&["ctr", "namespaces", "ls", "-q"]) {
        Ok(out) => out
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect::<Vec<_>>(),
        Err(e) => {
            info!("Could not list containerd namespaces ({e}), defaulting to k8s.io");
            vec!["k8s.io".to_string()]
        }
    };

    let mut all_clean = true;
    for namespace in &namespaces {
        cleanup_nydus_containers(namespace, containerd_snapshotter);
        if !cleanup_nydus_snapshots(namespace, containerd_snapshotter) {
            all_clean = false;
        }
    }
    all_clean
}

/// Remove all container records in this namespace whose snapshotter is the nydus instance.
///
/// Container records carry `snapshotter` and `snapshotKey` fields. After the nydus snapshots
/// are removed these records become dangling references. They do not cause pull failures but
/// can confuse container reconciliation if a previous CI run was aborted mid-test.
fn cleanup_nydus_containers(namespace: &str, containerd_snapshotter: &str) {
    // `ctr containers ls` output: ID  IMAGE  RUNTIME
    // We need to cross-reference with `ctr containers info <id>` to filter by snapshotter,
    // but that's expensive. Instead we rely on the fact that in the k8s.io namespace every
    // container using nydus will have been created by a pod that references it — we can
    // safely remove all containers whose snapshot key resolves to a nydus snapshot (i.e. any
    // container whose snapshotter field equals our snapshotter name). Since `ctr` does not
    // provide a direct --filter for snapshotter on the containers subcommand, we list all
    // container IDs, then inspect each one and remove those using the nydus snapshotter.
    let output = match utils::host_exec(&["ctr", "-n", namespace, "containers", "ls", "-q"]) {
        Ok(out) => out,
        Err(e) => {
            info!("Namespace {namespace}: cannot list containers ({e}), skipping container cleanup");
            return;
        }
    };

    let ids: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    if ids.is_empty() {
        return;
    }

    for id in &ids {
        // Inspect to check the snapshotter field; output is JSON.
        let info = match utils::host_exec(&["ctr", "-n", namespace, "containers", "info", id]) {
            Ok(out) => out,
            Err(_) => continue,
        };

        // Simple string search — avoids pulling in a JSON parser.
        // The field appears as `"Snapshotter": "nydus"` in the info output.
        let snapshotter_pattern = format!("\"Snapshotter\": \"{containerd_snapshotter}\"");
        if !info.contains(&snapshotter_pattern) {
            continue;
        }

        match utils::host_exec(&["ctr", "-n", namespace, "containers", "rm", id]) {
            Ok(_) => info!("Namespace {namespace}: removed nydus container '{id}'"),
            Err(e) => info!("Namespace {namespace}: could not remove container '{id}': {e}"),
        }
    }
}

/// Remove all snapshot records for the nydus snapshotter from this namespace, with retries.
///
/// Snapshot chains are linear (each layer is one snapshot parented on the previous), so an
/// image with N layers requires exactly N rounds — one leaf removal per round.  There is no
/// fixed round limit: the loop terminates naturally once the list is empty (all removed) or
/// makes zero progress (all remaining snapshots are actively mounted by running containers).
///
/// Returns `true` if all snapshots were removed, `false` if any remain (active workloads).
fn cleanup_nydus_snapshots(namespace: &str, containerd_snapshotter: &str) -> bool {
    let mut round: u32 = 0;
    loop {
        round += 1;

        // List all snapshots managed by this snapshotter in this namespace.
        let output = match utils::host_exec(&[
            "ctr",
            "-n",
            namespace,
            "snapshots",
            "--snapshotter",
            containerd_snapshotter,
            "list",
        ]) {
            Ok(out) => out,
            Err(e) => {
                info!("Namespace {namespace}: cannot list snapshots ({e}), skipping namespace");
                return true; // treat as clean: ctr unavailable, nothing we can do
            }
        };

        // Skip the header line; first whitespace-delimited token is the snapshot key.
        let keys: Vec<String> = output
            .lines()
            .skip(1)
            .filter_map(|l| {
                let k = l.split_whitespace().next()?;
                if k.is_empty() {
                    None
                } else {
                    Some(k.to_string())
                }
            })
            .collect();

        if keys.is_empty() {
            info!("Namespace {namespace}: no nydus snapshots remaining in containerd metadata");
            return true;
        }

        info!(
            "Namespace {namespace}: round {round}: removing {} snapshot(s)",
            keys.len()
        );

        let mut any_removed = false;
        for key in &keys {
            match utils::host_exec(&[
                "ctr",
                "-n",
                namespace,
                "snapshots",
                "--snapshotter",
                containerd_snapshotter,
                "rm",
                key,
            ]) {
                Ok(_) => {
                    any_removed = true;
                }
                Err(e) => {
                    // A snapshot cannot be removed when its children still exist.
                    // This is expected: we remove leaves first, parents in later rounds.
                    info!("Namespace {namespace}: could not remove snapshot '{key}': {e}");
                }
            }
        }

        if !any_removed {
            // No progress this round: all remaining snapshots are actively mounted.
            // Proceeding to wipe the data directory would corrupt running containers.
            warn!(
                "Namespace {namespace}: {} snapshot(s) remain after round {round} and none \
                 could be removed — active workloads are still using them",
                keys.len()
            );
            return false;
        }
    }
}

pub async fn install_nydus_snapshotter(config: &Config) -> Result<()> {
    info!("Deploying nydus-snapshotter");

    let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-snapshotter-{suffix}"),
        _ => "nydus-snapshotter".to_string(),
    };

    // The containerd proxy_plugins key for this nydus instance.
    let containerd_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-{suffix}"),
        _ => "nydus".to_string(),
    };

    // Clean up existing nydus-snapshotter state to ensure fresh start with new version.
    //
    // IMPORTANT: containerd metadata cleanup MUST happen before stopping the nydus service.
    // `ctr snapshots rm` goes through containerd's metadata layer which calls the nydus
    // gRPC backend to physically remove the snapshot. If the service is stopped first, the
    // backend call fails, leaving stale BoltDB records that cause subsequent image pulls to
    // fail with "target snapshot sha256:...: already exists" (see cleanup_containerd_nydus_snapshots).
    //
    // If cleanup returns false, active workloads are still using nydus snapshots.  Wiping
    // the data directory in that state would corrupt running containers, so we skip it and
    // let the new nydus instance start on top of the existing backend state.
    let all_clean = cleanup_containerd_nydus_snapshots(&containerd_snapshotter);

    // Stop the service now that the metadata has been cleaned.
    let _ = utils::host_systemctl(&["stop", &format!("{nydus_snapshotter}.service")]);

    // Only wipe the data directory when the metadata cleanup was complete.  If snapshots
    // remain (active workloads), preserve the backend so those containers are not broken.
    let nydus_data_dir = format!("/host/var/lib/{nydus_snapshotter}");
    if all_clean && Path::new(&nydus_data_dir).exists() {
        info!("Removing nydus data directory: {}", nydus_data_dir);
        fs::remove_dir_all(&nydus_data_dir).ok();
    } else if !all_clean {
        info!(
            "Skipping removal of nydus data directory (active workloads present): {}",
            nydus_data_dir
        );
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

    // Remove existing binaries before copying new ones.
    // This is crucial for atomic updates (same pattern as copy_artifacts in install.rs):
    // - If the file is in use (e.g., a running binary), the old inode stays alive
    // - The new copy creates a new inode
    // - Running processes keep using the old inode until they exit
    // - New processes use the new file immediately
    // Without this, fs::copy would fail with ETXTBSY ("Text file busy") if the
    // nydus-snapshotter service is still running from a previous installation.
    let grpc_binary = format!(
        "{}/nydus-snapshotter/containerd-nydus-grpc",
        config.host_install_dir
    );
    let overlayfs_binary = format!(
        "{}/nydus-snapshotter/nydus-overlayfs",
        config.host_install_dir
    );
    for binary in [&grpc_binary, &overlayfs_binary] {
        match fs::remove_file(binary) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    fs::copy(
        "/opt/kata-artifacts/nydus-snapshotter/containerd-nydus-grpc",
        &grpc_binary,
    )?;
    fs::copy(
        "/opt/kata-artifacts/nydus-snapshotter/nydus-overlayfs",
        &overlayfs_binary,
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

    let containerd_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("nydus-{suffix}"),
        _ => "nydus".to_string(),
    };

    // Clean up containerd metadata BEFORE disabling (and thus stopping) the service.
    // See install_nydus_snapshotter for the full explanation of why ordering matters.
    // If active workloads prevent a full cleanup, skip the data directory removal so
    // running containers are not broken.
    let all_clean = cleanup_containerd_nydus_snapshots(&containerd_snapshotter);

    utils::host_systemctl(&["disable", "--now", &format!("{nydus_snapshotter}.service")])?;

    fs::remove_file(format!(
        "/host/etc/systemd/system/{nydus_snapshotter}.service"
    ))
    .ok();
    fs::remove_dir_all(format!("{}/nydus-snapshotter", config.host_install_dir)).ok();

    let nydus_data_dir = format!("/host/var/lib/{nydus_snapshotter}");
    if all_clean && Path::new(&nydus_data_dir).exists() {
        fs::remove_dir_all(&nydus_data_dir).ok();
    } else if !all_clean {
        info!(
            "Skipping removal of nydus data directory (active workloads present): {}",
            nydus_data_dir
        );
    }

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
