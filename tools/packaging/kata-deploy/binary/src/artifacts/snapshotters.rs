// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Config, NYDUS_FOR_KATA_TEE};
use crate::runtime;
use crate::runtime::containerd;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::Result;
use log::{info, warn};
use std::fs;
use std::path::Path;

fn erofs_default_size(mode: Option<&str>) -> Result<&'static str> {
    match mode {
        Some("memory") => Ok("\"0\""),
        Some("disk") | None => Ok("\"10G\""),
        Some(other) => Err(anyhow::anyhow!(
            "Unsupported EROFS_SNAPSHOTTER_MODE: '{}'. Supported values: disk, memory",
            other
        )),
    }
}

pub async fn configure_erofs_snapshotter(config: &Config, configuration_file: &Path) -> Result<()> {
    info!("Configuring erofs-snapshotter");

    // "unmerged" mode keeps each image layer as its own per-layer `layer.erofs`
    // (containerd's default, non-fsmerged layout), which is the only layout the
    // Go runtime can consume. In the default "merged" mode we force containerd
    // to merge layers into a single `fsmeta.erofs`, which is runtime-rs only.
    let unmerged = config.erofs_merge_mode.as_deref() == Some("unmerged");

    // The Go runtime does not support fsmerged EROFS (fsmeta.erofs).
    // If the snapshotter handler mapping explicitly pairs a Go shim with
    // erofs in the (default) merged mode, that is a hard misconfiguration —
    // bail out so the operator fixes the mapping instead of hitting cryptic
    // runtime errors later. In "unmerged" mode the Go runtime is supported, so
    // skip this guard.
    if !unmerged {
        if let Some(mapping) = config.snapshotter_handler_mapping_for_arch.as_ref() {
            let mut go_shims_on_erofs = Vec::new();
            for entry in mapping.split(',') {
                let parts: Vec<&str> = entry.split(':').collect();
                if parts.len() == 2 && parts[1] == "erofs" && !utils::is_rust_shim(parts[0]) {
                    go_shims_on_erofs.push(parts[0].to_string());
                }
            }
            if !go_shims_on_erofs.is_empty() {
                warn!("##########################################################################");
                warn!("#                                                                        #");
                warn!("#  Go runtime shim(s) mapped to the erofs snapshotter:                   #");
                for s in &go_shims_on_erofs {
                    warn!("#    - {:<64} #", s);
                }
                warn!("#                                                                        #");
                warn!(
                    "#  The Go runtime does NOT support fsmerged EROFS (fsmeta.erofs).         #"
                );
                warn!("#  Only runtime-rs shims are supported with merged erofs. Set            #");
                warn!("#  EROFS_MERGE_MODE=unmerged to use the Go runtime with erofs.           #");
                warn!("#                                                                        #");
                warn!("##########################################################################");
                return Err(anyhow::anyhow!(
                    "erofs snapshotter: Go runtime shim(s) [{}] cannot be mapped to merged erofs. \
                     The Go runtime does not support fsmerged EROFS. \
                     Set EROFS_MERGE_MODE=unmerged, remove these shims from \
                     SNAPSHOTTER_HANDLER_MAPPING, or switch them to runtime-rs.",
                    go_shims_on_erofs.join(", ")
                ));
            }
        }
    }

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

    // dm-verity is orthogonal to rw-layer backing — it verifies lower (erofs)
    // layers via device-mapper regardless of whether the upper rw-layer lives on
    // disk or in memory.
    let use_dmverity = config.erofs_dmverity;
    let dmverity_mode = if use_dmverity { "\"on\"" } else { "\"off\"" };
    let enable_dmverity = if use_dmverity { "true" } else { "false" };

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

    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.snapshotter.v1.erofs\".dmverity_mode",
        dmverity_mode,
    )?;
    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.differ.v1.erofs\".enable_dmverity",
        enable_dmverity,
    )?;

    // Erofs differ plugin options (requires erofs-utils >= 1.8.2 on the host).
    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.differ.v1.erofs\".mkfs_options",
        "[\"-T0\",\"--mkfs-time\",\"--sort=none\"]",
    )?;
    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.differ.v1.erofs\".enable_tar_index",
        "false",
    )?;

    // Map EROFS_SNAPSHOTTER_MODE to containerd's default_size:
    // - "memory" uses an in-memory rw layer (default_size = 0)
    // - "disk" (or unset) uses a disk-backed rw layer (default_size = 10G)
    let default_size = erofs_default_size(config.erofs_snapshotter_mode.as_deref())?;
    toml_utils::set_toml_value(
        configuration_file,
        ".plugins.\"io.containerd.snapshotter.v1.erofs\".default_size",
        default_size,
    )?;
    // In the default "merged" mode, force containerd to merge all layers into a
    // single fsmeta.erofs (max_unmerged_layers = 0). In "unmerged" mode we delete
    // any previously-written value so each layer stays a separate layer.erofs,
    // which the Go runtime requires.
    //
    // Because kata-deploy edits the containerd config in place, switching from
    // merged to unmerged must actively remove the old `max_unmerged_layers = 0`
    // left behind by a previous install. Otherwise the stale `0` would keep
    // forcing the merged layout and break Go-runtime compatibility.
    if !unmerged {
        toml_utils::set_toml_value(
            configuration_file,
            ".plugins.\"io.containerd.snapshotter.v1.erofs\".max_unmerged_layers",
            "0",
        )?;
    } else {
        toml_utils::delete_toml_value(
            configuration_file,
            ".plugins.\"io.containerd.snapshotter.v1.erofs\".max_unmerged_layers",
        )?;
    }

    Ok(())
}

pub async fn configure_nydus_snapshotter(
    config: &Config,
    configuration_file: &Path,
    pluginid: &str,
) -> Result<()> {
    info!("Configuring {NYDUS_FOR_KATA_TEE}");

    let nydus = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{NYDUS_FOR_KATA_TEE}-{suffix}"),
        _ => NYDUS_FOR_KATA_TEE.to_string(),
    };

    let containerd_nydus = nydus.clone();

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
    toml_utils::set_toml_value(
        configuration_file,
        &format!(".proxy_plugins.\"{nydus}\".exports.root"),
        &format!("\"/var/lib/{nydus}\""),
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
        None => containerd::get_containerd_pluginid(&paths.config_file, runtime)?,
    };
    let pluginid =
        containerd::pluginid_for_snapshotter_annotations(runtime_plugin_id, &paths.config_file)?;

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
                Some(suffix) if !suffix.is_empty() => format!("{NYDUS_FOR_KATA_TEE}-{suffix}"),
                _ => NYDUS_FOR_KATA_TEE.to_string(),
            };

            utils::host_systemctl(&["restart", &nydus_snapshotter]).await?;
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

pub async fn install_nydus_snapshotter(config: &Config, runtime: &str) -> Result<()> {
    info!("Deploying {NYDUS_FOR_KATA_TEE}");

    let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{NYDUS_FOR_KATA_TEE}-{suffix}"),
        _ => NYDUS_FOR_KATA_TEE.to_string(),
    };

    // Stop the service if it is currently running so we can replace the binaries safely.
    let _ = utils::host_systemctl(&["stop", &format!("{nydus_snapshotter}.service")]).await;

    // Disable it as well: the [Install] section we are about to write may name a
    // different CRI unit than the one currently installed (e.g. after a kata-deploy
    // upgrade), and `systemctl disable` is the only thing that removes the stale
    // <old-cri-unit>.service.wants/ symlink.
    let _ = utils::host_systemctl(&["disable", &format!("{nydus_snapshotter}.service")]).await;

    // The nydus data directory (/var/lib/nydus-for-kata-tee) is intentionally preserved
    // across reinstalls.  Removing it would create a split-brain state: the nydus backend
    // would start empty while containerd's BoltDB (meta.db) still holds snapshot records
    // from the previous run.  Any subsequent image pull then fails with:
    //
    //   "unable to prepare extraction snapshot:
    //    target snapshot \"sha256:...\": already exists"
    //
    // because the metadata layer finds the target chainID in BoltDB and returns AlreadyExists
    // before the backend is consulted, but when Stat() delegates to the (now empty) backend
    // it gets NotFound — tripping the unpacker's retry loop.
    //
    // Cleaning up containerd's meta.db before wiping the dir was attempted, but that cleanup
    // itself requires the nydus gRPC service to be reachable (ctr snapshots rm calls the
    // backend).  If the service was stopped or crashed before the cleanup ran, the cleanup
    // silently fails and the split-brain state reappears.
    //
    // The correct invariant is simpler: meta.db and the nydus backend must always agree.
    // Preserving the data directory across reinstalls guarantees this at zero cost.
    // Any stale snapshots from previous workloads are naturally garbage-collected by
    // containerd once the images that reference them are removed.

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
            "{}/{NYDUS_FOR_KATA_TEE}/nydus-overlayfs",
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
            "{}/{NYDUS_FOR_KATA_TEE}/containerd-nydus-grpc",
            &config
                .host_install_dir
                .strip_prefix("/host")
                .unwrap_or(&config.host_install_dir)
        ),
    );
    service_content = service_content.replace(
        "@CONFIG_GUEST_PULLING@",
        &format!(
            "{}/{NYDUS_FOR_KATA_TEE}/config-guest-pulling.toml",
            &config
                .host_install_dir
                .strip_prefix("/host")
                .unwrap_or(&config.host_install_dir)
        ),
    );

    // Hook the snapshotter onto whichever unit actually runs containerd on this node.
    let cri_service = runtime::cri_systemd_unit(runtime);
    info!("Binding {nydus_snapshotter}.service to {cri_service}");
    service_content = service_content.replace("@CRI_SERVICE@", &cri_service);

    fs::create_dir_all(format!("{}/{NYDUS_FOR_KATA_TEE}", config.host_install_dir))?;

    // Remove existing binaries before copying new ones.
    // This is crucial for atomic updates (same pattern as copy_artifacts in install.rs):
    // - If the file is in use (e.g., a running binary), the old inode stays alive
    // - The new copy creates a new inode
    // - Running processes keep using the old inode until they exit
    // - New processes use the new file immediately
    // Without this, fs::copy would fail with ETXTBSY ("Text file busy") if the
    // nydus-for-kata-tee service is still running from a previous installation.
    let grpc_binary = format!(
        "{}/{NYDUS_FOR_KATA_TEE}/containerd-nydus-grpc",
        config.host_install_dir
    );
    let overlayfs_binary = format!(
        "{}/{NYDUS_FOR_KATA_TEE}/nydus-overlayfs",
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
            "{}/{NYDUS_FOR_KATA_TEE}/config-guest-pulling.toml",
            config.host_install_dir
        ),
        config_content,
    )?;

    fs::write(
        format!("/host/etc/systemd/system/{nydus_snapshotter}.service"),
        service_content,
    )?;

    utils::host_systemctl(&["daemon-reload"]).await?;
    utils::host_systemctl(&["enable", &format!("{nydus_snapshotter}.service")]).await?;

    Ok(())
}

pub async fn uninstall_nydus_snapshotter(config: &Config) -> Result<()> {
    info!("Removing deployed {NYDUS_FOR_KATA_TEE}");

    let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{NYDUS_FOR_KATA_TEE}-{suffix}"),
        _ => NYDUS_FOR_KATA_TEE.to_string(),
    };

    utils::host_systemctl(&["disable", "--now", &format!("{nydus_snapshotter}.service")]).await?;

    fs::remove_file(format!(
        "/host/etc/systemd/system/{nydus_snapshotter}.service"
    ))
    .ok();
    fs::remove_dir_all(format!("{}/{NYDUS_FOR_KATA_TEE}", config.host_install_dir)).ok();

    // The nydus data directory (/var/lib/nydus-for-kata-tee) is intentionally preserved.
    // See install_nydus_snapshotter for the full explanation: meta.db and the nydus backend
    // must always agree, and the only way to guarantee that without complex, fragile cleanup
    // logic is to never remove the data directory.  After uninstall, containerd is
    // reconfigured without the nydus proxy_plugins entry and restarted, so the remaining
    // snapshot records in meta.db are completely dormant — nothing will use them.  If nydus
    // is reinstalled later the data directory is still present and both sides remain in sync.

    utils::host_systemctl(&["daemon-reload"]).await?;

    Ok(())
}

pub async fn install_snapshotter(snapshotter: &str, config: &Config, runtime: &str) -> Result<()> {
    match snapshotter {
        "erofs" => {
            // erofs is a containerd built-in snapshotter, no installation needed
        }
        "nydus" => {
            install_nydus_snapshotter(config, runtime).await?;
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

#[cfg(test)]
mod tests {
    use super::erofs_default_size;
    use rstest::rstest;

    #[rstest]
    #[case(None, "\"10G\"")]
    #[case(Some("disk"), "\"10G\"")]
    #[case(Some("memory"), "\"0\"")]
    fn test_erofs_default_size(#[case] mode: Option<&str>, #[case] expected: &str) {
        assert_eq!(erofs_default_size(mode).unwrap(), expected);
    }

    #[test]
    fn test_erofs_default_size_rejects_unknown_mode() {
        let error = erofs_default_size(Some("unknown")).unwrap_err();
        assert!(error
            .to_string()
            .contains("Unsupported EROFS_SNAPSHOTTER_MODE"));
    }
}
