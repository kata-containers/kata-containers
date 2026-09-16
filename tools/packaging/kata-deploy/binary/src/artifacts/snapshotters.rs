// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Config, NYDUS_FOR_KATA_TEE};
use crate::runtime;
use crate::runtime::containerd;
use crate::runtime::node_config;
use crate::utils;
use crate::utils::toml as toml_utils;
use anyhow::Result;
use log::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};

const EROFS_SNAPSHOTTER: &str = ".plugins.\"io.containerd.snapshotter.v1.erofs\"";
const EROFS_DIFFER: &str = ".plugins.\"io.containerd.differ.v1.erofs\"";

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

fn node_configures_erofs(sources: &[PathBuf]) -> bool {
    [EROFS_SNAPSHOTTER, EROFS_DIFFER]
        .iter()
        .any(|table| node_config::table_stated(sources, table))
}

/// Absent keys mean off, which is also what the layers already there carry.
fn node_uses_dmverity(sources: &[PathBuf]) -> bool {
    let mode = node_config::states(sources, &format!("{EROFS_SNAPSHOTTER}.dmverity_mode"));
    let differ = node_config::states(sources, &format!("{EROFS_DIFFER}.enable_dmverity"));

    mode.is_some_and(|(_, mode)| mode != "off")
        || differ.is_some_and(|(_, enabled)| enabled == "true")
}

/// Change one of these and containerd can no longer mount layers built to it.
fn erofs_conflicts_with_node(sources: &[PathBuf], dmverity: bool, unmerged: bool) -> Vec<String> {
    if !node_configures_erofs(sources) {
        return Vec::new();
    }

    let mut conflicts = Vec::new();
    let on_off = |on: bool| if on { "on" } else { "off" };

    if node_uses_dmverity(sources) != dmverity {
        conflicts.push(format!(
            "dm-verity is {} on this node, and this install would turn it {}",
            on_off(!dmverity),
            on_off(dmverity)
        ));
    }

    // Absent, containerd picks the layout - and so would we, unmerged.
    if let Some((file, stated)) =
        node_config::states(sources, &format!("{EROFS_SNAPSHOTTER}.max_unmerged_layers"))
    {
        let node_merges = stated == "0";
        if node_merges == unmerged {
            conflicts.push(format!(
                "max_unmerged_layers is {stated} in {}, which {} layers, and this install wants \
                 them {}",
                file.display(),
                if node_merges { "merges" } else { "keeps" },
                if unmerged { "unmerged" } else { "merged" }
            ));
        }
    }

    conflicts
}

pub async fn configure_erofs_snapshotter(
    config: &Config,
    configuration_file: &Path,
    node_sources: &[PathBuf],
) -> Result<()> {
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

    // dm-verity is orthogonal to rw-layer backing — it verifies lower (erofs)
    // layers via device-mapper regardless of whether the upper rw-layer lives on
    // disk or in memory.
    let use_dmverity = config.erofs_dmverity;
    let dmverity_mode = if use_dmverity { "\"on\"" } else { "\"off\"" };
    let enable_dmverity = if use_dmverity { "true" } else { "false" };

    // Map EROFS_SNAPSHOTTER_MODE to containerd's default_size:
    // - "memory" uses an in-memory rw layer (default_size = 0)
    // - "disk" (or unset) uses a disk-backed rw layer (default_size = 10G)
    let default_size = erofs_default_size(config.erofs_snapshotter_mode.as_deref())?;

    let conflicts = erofs_conflicts_with_node(node_sources, config.erofs_dmverity, unmerged);
    anyhow::ensure!(
        conflicts.is_empty(),
        "erofs snapshotter: this node already sets erofs up, and this install would change it: \
         {}. Layers the node already pulled were built to its settings and stop mounting once \
         those change. Match them in the chart values, drop \"erofs\" from snapshotter.setup to \
         leave the node's own setup alone, or clear the erofs snapshotter state first.",
        conflicts.join("; ")
    );

    // The rest only shape new layers, so we take them.
    node_config::warn_about_overrides(
        node_sources,
        &[
            (
                ".plugins.\"io.containerd.cri.v1.images\".discard_unpacked_layers",
                "false".to_string(),
            ),
            (
                ".plugins.\"io.containerd.service.v1.diff-service\".default",
                "[\"erofs\",\"walking\"]".to_string(),
            ),
            (
                ".plugins.\"io.containerd.snapshotter.v1.erofs\".enable_fsverity",
                "true".to_string(),
            ),
            (
                ".plugins.\"io.containerd.snapshotter.v1.erofs\".set_immutable",
                "true".to_string(),
            ),
            (
                ".plugins.\"io.containerd.snapshotter.v1.erofs\".default_size",
                default_size.to_string(),
            ),
            (
                ".plugins.\"io.containerd.differ.v1.erofs\".mkfs_options",
                "[\"-T0\",\"--mkfs-time\",\"--sort=none\"]".to_string(),
            ),
            (
                ".plugins.\"io.containerd.differ.v1.erofs\".enable_tar_index",
                "false".to_string(),
            ),
        ],
    );

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

/// containerd's meta.db names the root its snapshots were prepared under, the
/// split-brain [`install_nydus_snapshotter`] keeps the data directory to avoid.
fn nydus_conflicts_with_node(sources: &[PathBuf], nydus: &str, root: &str) -> Vec<String> {
    let exports_root = format!(".proxy_plugins.\"{nydus}\".exports.root");

    match node_config::states(sources, &exports_root) {
        Some((file, stated)) if stated != root => vec![format!(
            "{nydus} exports root {stated} in {}, and this install would point it at {root}",
            file.display()
        )],
        _ => Vec::new(),
    }
}

pub async fn configure_nydus_snapshotter(
    config: &Config,
    configuration_file: &Path,
    pluginid: &str,
    node_sources: &[PathBuf],
) -> Result<()> {
    info!("Configuring {NYDUS_FOR_KATA_TEE}");

    let nydus = match config.multi_install_suffix.as_ref() {
        Some(suffix) if !suffix.is_empty() => format!("{NYDUS_FOR_KATA_TEE}-{suffix}"),
        _ => NYDUS_FOR_KATA_TEE.to_string(),
    };

    let containerd_nydus = nydus.clone();
    let root = format!("/var/lib/{nydus}");
    let socket = format!("/run/{containerd_nydus}/containerd-nydus-grpc.sock");
    let annotations = format!(".plugins.{pluginid}.disable_snapshot_annotations");

    let conflicts = nydus_conflicts_with_node(node_sources, &nydus, &root);
    anyhow::ensure!(
        conflicts.is_empty(),
        "{NYDUS_FOR_KATA_TEE}: this node already points the proxy plugin somewhere else: {}. The \
         snapshots containerd recorded are in the root it named, and a backend that does not hold \
         them fails the pulls that follow. Match that root, drop \"nydus\" from \
         snapshotter.setup, or clear the snapshot state first.",
        conflicts.join("; ")
    );

    node_config::warn_about_overrides(
        node_sources,
        &[
            (annotations.clone(), "false".to_string()),
            (
                format!(".proxy_plugins.\"{nydus}\".type"),
                "\"snapshot\"".to_string(),
            ),
            (
                format!(".proxy_plugins.\"{nydus}\".address"),
                format!("\"{socket}\""),
            ),
        ],
    );

    toml_utils::set_toml_value(configuration_file, &annotations, "false")?;

    toml_utils::set_toml_value(
        configuration_file,
        &format!(".proxy_plugins.\"{nydus}\".type"),
        "\"snapshot\"",
    )?;
    toml_utils::set_toml_value(
        configuration_file,
        &format!(".proxy_plugins.\"{nydus}\".address"),
        &format!("\"{socket}\""),
    )?;
    toml_utils::set_toml_value(
        configuration_file,
        &format!(".proxy_plugins.\"{nydus}\".exports.root"),
        &format!("\"{root}\""),
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
        let base_path = Path::new(&paths.drop_in_file).to_path_buf();

        log::debug!("Snapshotter using drop-in config file: {:?}", base_path);
        base_path
    } else {
        log::debug!("Snapshotter using main config file: {}", paths.config_file);
        Path::new(&paths.config_file).to_path_buf()
    };

    let node_sources = node_config::sources(&paths, config.multi_install_suffix.as_deref());

    match snapshotter {
        "nydus" => {
            configure_nydus_snapshotter(config, &configuration_file, pluginid, &node_sources)
                .await?;

            let nydus_snapshotter = match config.multi_install_suffix.as_ref() {
                Some(suffix) if !suffix.is_empty() => format!("{NYDUS_FOR_KATA_TEE}-{suffix}"),
                _ => NYDUS_FOR_KATA_TEE.to_string(),
            };

            utils::host_systemctl(&["restart", &nydus_snapshotter]).await?;
        }
        "erofs" => {
            configure_erofs_snapshotter(config, &configuration_file, &node_sources).await?;
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
            &config.host_install_dir
        ),
    );

    let mut service_content = fs::read_to_string(nydus_snapshotter_service)?;
    service_content = service_content.replace(
        "@CONTAINERD_NYDUS_GRPC_BINARY@",
        &format!(
            "{}/{NYDUS_FOR_KATA_TEE}/containerd-nydus-grpc",
            &config.host_install_dir
        ),
    );
    service_content = service_content.replace(
        "@CONFIG_GUEST_PULLING@",
        &format!(
            "{}/{NYDUS_FOR_KATA_TEE}/config-guest-pulling.toml",
            &config.host_install_dir
        ),
    );

    // Hook the snapshotter onto whichever unit actually runs containerd on this node.
    let cri_service = runtime::cri_systemd_unit_for(runtime, config.cri_service_name.as_deref());
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
        format!("/etc/systemd/system/{nydus_snapshotter}.service"),
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

    let service = format!("/etc/systemd/system/{nydus_snapshotter}.service");
    if Path::new(&service).exists() {
        utils::host_systemctl(&["disable", "--now", &format!("{nydus_snapshotter}.service")])
            .await?;
        fs::remove_file(service).ok();
    }
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
    use super::{erofs_conflicts_with_node, erofs_default_size, nydus_conflicts_with_node};
    use rstest::rstest;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn node_config(body: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, format!("version = 3\n{body}")).unwrap();
        (dir, path)
    }

    #[rstest]
    #[case::empty("")]
    #[case::unrelated_plugin(
        "[plugins.'io.containerd.snapshotter.v1.overlayfs']\nroot_path = '/x'\n"
    )]
    fn a_node_without_its_own_erofs_setup_is_left_to_us(#[case] body: &str) {
        let (_dir, path) = node_config(body);

        assert!(erofs_conflicts_with_node(std::slice::from_ref(&path), true, false).is_empty());
        assert!(erofs_conflicts_with_node(std::slice::from_ref(&path), false, true).is_empty());
    }

    #[test]
    fn turning_dmverity_on_under_a_node_that_ran_without_it_conflicts() {
        let (_dir, path) =
            node_config("[plugins.'io.containerd.snapshotter.v1.erofs']\nenable_fsverity = true\n");

        let conflicts = erofs_conflicts_with_node(std::slice::from_ref(&path), true, false);

        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].contains("dm-verity"), "{conflicts:?}");
    }

    #[rstest]
    #[case::the_node_states_it("dmverity_mode = 'on'\nenable_fsverity = true\n", true, 0)]
    #[case::the_install_would_drop_it("dmverity_mode = 'on'\n", false, 1)]
    #[case::auto_counts_as_on("dmverity_mode = 'auto'\n", true, 0)]
    #[case::off_is_off("dmverity_mode = 'off'\n", false, 0)]
    fn dmverity_has_to_agree_with_the_node(
        #[case] snapshotter_body: &str,
        #[case] dmverity: bool,
        #[case] expected: usize,
    ) {
        let (_dir, path) = node_config(&format!(
            "[plugins.'io.containerd.snapshotter.v1.erofs']\n{snapshotter_body}"
        ));

        assert_eq!(
            erofs_conflicts_with_node(std::slice::from_ref(&path), dmverity, false).len(),
            expected
        );
    }

    #[test]
    fn the_differs_dmverity_counts_too() {
        let (_dir, path) =
            node_config("[plugins.'io.containerd.differ.v1.erofs']\nenable_dmverity = true\n");

        assert!(erofs_conflicts_with_node(std::slice::from_ref(&path), true, false).is_empty());
        assert_eq!(
            erofs_conflicts_with_node(std::slice::from_ref(&path), false, false).len(),
            1
        );
    }

    #[rstest]
    #[case::node_merges_and_so_do_we("max_unmerged_layers = 0", false, 0)]
    #[case::node_merges_and_we_would_not("max_unmerged_layers = 0", true, 1)]
    #[case::node_keeps_layers_and_we_would_merge("max_unmerged_layers = 4", false, 1)]
    #[case::node_keeps_layers_and_so_do_we("max_unmerged_layers = 4", true, 0)]
    #[case::node_states_nothing("enable_fsverity = true", false, 0)]
    fn the_layer_layout_has_to_agree_with_the_node(
        #[case] stated: &str,
        #[case] unmerged: bool,
        #[case] expected: usize,
    ) {
        let (_dir, path) = node_config(&format!(
            "[plugins.'io.containerd.snapshotter.v1.erofs']\n{stated}\n"
        ));

        let conflicts = erofs_conflicts_with_node(std::slice::from_ref(&path), false, unmerged);

        assert_eq!(conflicts.len(), expected, "{conflicts:?}");
    }

    #[test]
    fn a_missing_node_config_is_not_a_conflict() {
        let sources = [PathBuf::from("/nonexistent")];

        assert!(erofs_conflicts_with_node(&sources, true, false).is_empty());
        assert!(nydus_conflicts_with_node(&sources, "nydus-for-kata-tee", "/var/lib/x").is_empty());
    }

    #[rstest]
    #[case::the_same_root("/var/lib/nydus-for-kata-tee", 0)]
    #[case::another_root("/srv/nydus", 1)]
    fn a_nydus_root_of_the_nodes_own_is_not_repointed(
        #[case] stated: &str,
        #[case] expected: usize,
    ) {
        let (_dir, path) = node_config(&format!(
            "[proxy_plugins.'nydus-for-kata-tee'.exports]\nroot = '{stated}'\n"
        ));

        let conflicts =
            nydus_conflicts_with_node(&[path], "nydus-for-kata-tee", "/var/lib/nydus-for-kata-tee");

        assert_eq!(conflicts.len(), expected, "{conflicts:?}");
    }

    #[test]
    fn a_node_with_no_nydus_plugin_is_left_to_us() {
        let (_dir, path) = node_config("version = 3\n");

        assert!(nydus_conflicts_with_node(
            &[path],
            "nydus-for-kata-tee",
            "/var/lib/nydus-for-kata-tee"
        )
        .is_empty());
    }

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
