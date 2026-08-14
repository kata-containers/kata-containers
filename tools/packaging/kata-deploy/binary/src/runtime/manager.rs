// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::utils;
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::containerd;
use super::crio;
use super::lifecycle;

/// List of container runtimes that are containerd-based
const CONTAINERD_BASED_RUNTIMES: &[&str] = &[
    "containerd",
    "k3s",
    "k3s-agent",
    "rke2-agent",
    "rke2-server",
    "k0s-controller",
    "k0s-worker",
    "microk8s",
];

/// Runtimes that don't support containerd drop-in configuration files.
///
/// K3s/RKE2 can use drop-in when the rendered config already imports the
/// versioned drop-in dir; we check that in get_containerd_paths and bail otherwise.
const RUNTIMES_WITHOUT_CONTAINERD_DROP_IN_SUPPORT: &[&str] = &["crio"];

fn is_containerd_based(runtime: &str) -> bool {
    CONTAINERD_BASED_RUNTIMES.contains(&runtime)
}

async fn is_unit_active(unit: &str) -> bool {
    utils::host_systemctl(&["is-active", "--quiet", unit])
        .await
        .is_ok()
}

pub async fn get_container_runtime(config: &Config) -> Result<String> {
    let runtime_version = config
        .resolve_container_runtime_version()
        .await
        .context("Failed to get container runtime version")?;

    // Asked of the host rather than of the `microk8s.io/cluster` node label, so
    // that a pod holding no Kubernetes credentials can still tell: microk8s runs
    // containerd as a snap daemon, and the version string kubelet reports says
    // only "containerd".
    if is_unit_active(&cri_systemd_unit("microk8s")).await {
        return Ok("microk8s".to_string());
    }

    if runtime_version.contains("cri-o") || runtime_version.contains("crio") {
        return Ok("crio".to_string());
    }

    // Detect k3s/rke2 via systemd services rather than the containerd version
    // string, which no longer reliably contains "k3s" in newer releases
    // (e.g. "containerd://2.2.2-bd1.34").
    if is_unit_active("rke2-agent").await {
        return Ok("rke2-agent".to_string());
    }
    if is_unit_active("rke2-server").await {
        return Ok("rke2-server".to_string());
    }
    if is_unit_active("k3s-agent").await {
        return Ok("k3s-agent".to_string());
    }
    if is_unit_active("k3s").await {
        return Ok("k3s".to_string());
    }

    if is_unit_active("k0scontroller").await {
        return Ok("k0s-controller".to_string());
    }
    if is_unit_active("k0sworker").await {
        return Ok("k0s-worker".to_string());
    }

    // Default: extract runtime name from version string
    let runtime = runtime_version
        .split(':')
        .next()
        .unwrap_or("containerd")
        .to_string();

    Ok(runtime)
}

/// Returns the systemd unit that runs the node's CRI runtime.
///
/// For most runtimes the detected runtime name doubles as the unit name, but k3s,
/// RKE2 and k0s embed containerd in their own daemon instead of running a
/// standalone `containerd.service`, and microk8s ships containerd as a snap
/// daemon.  Note that the k0s units carry no dash: the `k0s-controller` and
/// `k0s-worker` names above are ours, the units are `k0scontroller`/`k0sworker`.
pub fn cri_systemd_unit(runtime: &str) -> String {
    match runtime {
        "k0s-controller" => "k0scontroller.service".to_string(),
        "k0s-worker" => "k0sworker.service".to_string(),
        "microk8s" => "snap.microk8s.daemon-containerd.service".to_string(),
        _ => format!("{runtime}.service"),
    }
}

/// Returns true if containerRuntimeVersion (e.g. "containerd://2.1.5-k3s1", "containerd://2.2.2-bd1.34") indicates
/// containerd 2.x or newer, false for 1.x or unparseable. Used for drop-in support
/// and for K3s/RKE2 template selection (config-v3.toml.tmpl vs config.toml.tmpl).
pub fn containerd_version_is_2_or_newer(runtime_version: &str) -> bool {
    let version_re = match Regex::new(r"containerd://(\d+)\.(\d+)") {
        Ok(r) => r,
        Err(_) => return false,
    };
    if let Some(caps) = version_re.captures(runtime_version) {
        if let Ok(major) = caps.get(1).unwrap().as_str().parse::<u32>() {
            return major >= 2;
        }
    }
    false
}

/// Returns true if containerRuntimeVersion (e.g. "containerd://2.2.0-k3s1") indicates
/// containerd 2.2.0 or newer, false otherwise. Used to check if conf.d auto-import is supported
/// (containerd >= 2.2.0 always imports /etc/containerd/conf.d/).
pub fn containerd_version_is_2_2_or_newer(runtime_version: &str) -> bool {
    let version_re = match Regex::new(r"containerd://(\d+)\.(\d+)") {
        Ok(r) => r,
        Err(_) => return false,
    };
    if let Some(caps) = version_re.captures(runtime_version) {
        if let (Ok(major), Ok(minor)) = (
            caps.get(1).unwrap().as_str().parse::<u32>(),
            caps.get(2).unwrap().as_str().parse::<u32>(),
        ) {
            return major > 2 || (major == 2 && minor >= 2);
        }
    }
    // Also support bare version strings like "2.2.0"
    let version_re = match Regex::new(r"^(\d+)\.(\d+)") {
        Ok(r) => r,
        Err(_) => return false,
    };
    if let Some(caps) = version_re.captures(runtime_version) {
        if let (Ok(major), Ok(minor)) = (
            caps.get(1).unwrap().as_str().parse::<u32>(),
            caps.get(2).unwrap().as_str().parse::<u32>(),
        ) {
            return major > 2 || (major == 2 && minor >= 2);
        }
    }
    false
}

/// Pure version of the drop-in capability check. Takes an optional containerd version string
/// instead of making its own k8s API call. Used by `get_containerd_paths` which already has the
/// version available.
pub fn is_containerd_capable_of_drop_in(runtime: &str, runtime_version: Option<&str>) -> bool {
    if RUNTIMES_WITHOUT_CONTAINERD_DROP_IN_SUPPORT.contains(&runtime) {
        return false;
    }

    // k0s always supports drop-in files (auto-loads from containerd.d/)
    if runtime == "k0s-worker" || runtime == "k0s-controller" {
        return true;
    }

    // Check containerd version - only 2.0+ supports drop-in files properly
    runtime_version
        .map(containerd_version_is_2_or_newer)
        .unwrap_or(false)
}

pub async fn is_containerd_capable_of_using_drop_in_files(
    config: &Config,
    runtime: &str,
) -> Result<bool> {
    if RUNTIMES_WITHOUT_CONTAINERD_DROP_IN_SUPPORT.contains(&runtime) {
        return Ok(false);
    }
    // k0s always supports drop-in files (auto-loads from containerd.d/)
    let runtime_version = if runtime == "k0s-worker" || runtime == "k0s-controller" {
        None
    } else {
        Some(config.resolve_container_runtime_version().await?)
    };

    Ok(is_containerd_capable_of_drop_in(
        runtime,
        runtime_version.as_deref(),
    ))
}

pub async fn configure_cri_runtime(config: &Config, runtime: &str) -> Result<()> {
    if runtime == "crio" {
        crio::configure_crio(config).await?;
    } else if is_containerd_based(runtime) {
        containerd::configure_containerd(config, runtime).await?;
    } else {
        return Err(anyhow::anyhow!("Unsupported runtime: {runtime}"));
    }

    Ok(())
}

/// What the kata CRI configuration for a runtime looked like at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriConfigSnapshot {
    fingerprint: String,
    written_at: Option<SystemTime>,
}

impl CriConfigSnapshot {
    pub fn written_at(&self) -> Option<SystemTime> {
        self.written_at
    }
}

/// Snapshot the current on-disk kata CRI configuration for `runtime`.
///
/// The config writes are idempotent, so comparing a snapshot from before a
/// re-apply with one from after says whether the runtime has to be restarted to
/// pick anything up.
pub async fn cri_config_snapshot(config: &Config, runtime: &str) -> Option<CriConfigSnapshot> {
    let files = if runtime == "crio" {
        crio::kata_cri_config_files(config)
    } else if is_containerd_based(runtime) {
        containerd::kata_cri_config_files(config, runtime).await?
    } else {
        return None;
    };

    snapshot_files(&files)
}

/// Fold `files` into a single snapshot, in order.
///
/// A missing first entry means kata was never configured here. The rest count
/// as empty when absent, so a user drop-in appearing still reads as a change.
fn snapshot_files(files: &[PathBuf]) -> Option<CriConfigSnapshot> {
    let (primary, rest) = files.split_first()?;

    let mut fingerprint = read_for_fingerprint(primary)?;
    let mut written_at = mtime(primary);

    for file in rest {
        fingerprint.push_str(&read_for_fingerprint(file).unwrap_or_default());
        written_at = written_at.max(mtime(file));
    }

    Some(CriConfigSnapshot {
        fingerprint,
        written_at,
    })
}

/// Path and content, framed so that content moving between files still reads as
/// a change.
fn read_for_fingerprint(file: &Path) -> Option<String> {
    let content = fs::read_to_string(file).ok()?;

    Some(format!(
        "{}\n{}\n{content}\n",
        file.display(),
        content.len()
    ))
}

fn mtime(file: &Path) -> Option<SystemTime> {
    fs::metadata(file).ok()?.modified().ok()
}

/// Remove CRI runtime configuration (containerd/crio config files) without restarting.
pub async fn cleanup_cri_runtime_config(config: &Config, runtime: &str) -> Result<()> {
    log::info!(
        "cleanup_cri_runtime_config: Starting cleanup for runtime={}",
        runtime
    );

    if runtime == "crio" {
        log::info!("cleanup_cri_runtime_config: Cleaning up crio");
        crio::cleanup_crio(config).await?;
        log::info!("cleanup_cri_runtime_config: Successfully cleaned up crio");
    } else if is_containerd_based(runtime) {
        log::info!("cleanup_cri_runtime_config: Cleaning up containerd");
        containerd::cleanup_containerd(config, runtime).await?;
        log::info!("cleanup_cri_runtime_config: Successfully cleaned up containerd");
    } else {
        return Err(anyhow::anyhow!("Unsupported runtime: {runtime}"));
    }

    log::info!("cleanup_cri_runtime_config: Cleanup completed");
    Ok(())
}

/// Restart the CRI runtime and wait for it to come back.
///
/// `staged` picks the wait: a staged per-node Job has no Kubernetes credentials
/// and waits on the systemd unit, the DaemonSet waits for the node to go Ready.
pub async fn restart_and_wait_for_ready(
    config: &Config,
    runtime: &str,
    staged: bool,
) -> Result<()> {
    log::info!("restart_and_wait_for_ready: Restarting runtime");
    lifecycle::restart_cri_runtime(config, runtime).await?;
    log::info!("restart_and_wait_for_ready: Successfully restarted runtime");

    if staged {
        if !matches!(runtime, "k0s-worker" | "k0s-controller") {
            log::info!(
                "restart_and_wait_for_ready: Waiting for the CRI runtime unit (timeout: 300s)"
            );
            lifecycle::wait_till_cri_unit_active(runtime, 300).await?;
        }
        return Ok(());
    }

    log::info!("restart_and_wait_for_ready: Waiting for node to become ready (timeout: 300s)");
    lifecycle::wait_till_node_is_ready_timeout(config, Some(300)).await?;
    log::info!("restart_and_wait_for_ready: Node is ready");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::tempdir;

    // --- snapshot_files ---
    //
    // What install_stage_cri decides on: equal fingerprints across per-node Job
    // retries mean this config is already on disk, so the retry can converge
    // instead of restarting the runtime (and getting killed) again.

    #[test]
    fn absent_primary_file_is_none() {
        let dir = tempdir().unwrap();

        assert_eq!(
            snapshot_files(&[dir.path().join("kata-deploy.toml")]),
            None,
            "no config on disk yet must read as None (fresh install -> restart)"
        );
    }

    #[test]
    fn a_secondary_file_appearing_is_a_change() {
        let dir = tempdir().unwrap();
        let drop_in = dir.path().join("kata-deploy.toml");
        let user_drop_in = dir.path().join("zz-kata-deploy-user.toml");
        fs::write(&drop_in, "kata").unwrap();

        let files = [drop_in, user_drop_in.clone()];
        let before = snapshot_files(&files).unwrap();

        // A user drop-in added by an upgrade leaves the kata drop-in untouched,
        // so this is exactly the change a single-file snapshot used to miss.
        fs::write(&user_drop_in, "user").unwrap();
        assert_ne!(
            before.fingerprint,
            snapshot_files(&files).unwrap().fingerprint
        );
    }

    #[test]
    fn identical_content_fingerprints_the_same() {
        let dir = tempdir().unwrap();
        let drop_in = dir.path().join("kata-deploy.toml");
        let main_config = dir.path().join("config.toml");
        fs::write(&drop_in, "kata").unwrap();
        fs::write(&main_config, "imports = [\"kata-deploy.toml\"]").unwrap();

        let files = [drop_in, main_config.clone()];
        let before = snapshot_files(&files).unwrap();

        assert_eq!(
            before.fingerprint,
            snapshot_files(&files).unwrap().fingerprint
        );

        // Losing the import means containerd no longer reads the drop-in, even
        // though the drop-in itself is byte-for-byte what we want.
        fs::write(&main_config, "imports = []").unwrap();
        assert_ne!(
            before.fingerprint,
            snapshot_files(&files).unwrap().fingerprint
        );
    }

    #[test]
    fn written_at_is_the_newest_file() {
        let dir = tempdir().unwrap();
        let drop_in = dir.path().join("kata-deploy.toml");
        let user_drop_in = dir.path().join("zz-kata-deploy-user.toml");
        fs::write(&drop_in, "kata").unwrap();
        fs::write(&user_drop_in, "user").unwrap();

        let newest = mtime(&drop_in).max(mtime(&user_drop_in));
        assert_eq!(
            snapshot_files(&[drop_in, user_drop_in])
                .unwrap()
                .written_at(),
            newest,
            "the restart has to be newer than the last write, whichever file it landed in"
        );
    }

    // --- containerd_version_is_2_or_newer ---

    #[rstest]
    #[case("containerd://2.0.0", true)]
    #[case("containerd://2.1.5", true)]
    #[case("containerd://2.1.5-k3s1", true)]
    #[case("containerd://2.2.2-bd1.34", true)]
    #[case("containerd://2.2.0", true)]
    #[case("containerd://2.3.1", true)]
    #[case("containerd://2.0.0-rc.1", true)]
    #[case("containerd://1.6.28", false)]
    #[case("containerd://1.7.15", false)]
    #[case("containerd://1.7.0", false)]
    #[case("containerd://", false)]
    #[case("1.7.0", false)]
    #[case("not-a-version", false)]
    fn test_containerd_version_is_2_or_newer(#[case] version: &str, #[case] expected: bool) {
        assert_eq!(
            containerd_version_is_2_or_newer(version),
            expected,
            "version: {}",
            version
        );
    }

    // --- cri_systemd_unit ---

    /// The runtime name doubles as the unit name everywhere except k0s (no dash in
    /// the unit) and microk8s (snap daemon), which are the cases worth pinning down.
    #[rstest]
    #[case::vanilla_containerd("containerd", "containerd.service")]
    #[case::crio("crio", "crio.service")]
    #[case::k3s_server("k3s", "k3s.service")]
    #[case::k3s_agent("k3s-agent", "k3s-agent.service")]
    #[case::rke2_server("rke2-server", "rke2-server.service")]
    #[case::rke2_agent("rke2-agent", "rke2-agent.service")]
    #[case::k0s_controller_unit_has_no_dash("k0s-controller", "k0scontroller.service")]
    #[case::k0s_worker_unit_has_no_dash("k0s-worker", "k0sworker.service")]
    #[case::microk8s_containerd_is_a_snap_daemon(
        "microk8s",
        "snap.microk8s.daemon-containerd.service"
    )]
    #[case::unknown_runtime_falls_back_to_its_own_name("something-else", "something-else.service")]
    fn test_cri_systemd_unit(#[case] runtime: &str, #[case] expected: &str) {
        assert_eq!(cri_systemd_unit(runtime), expected, "runtime: {}", runtime);
    }

    // --- is_containerd_capable_of_drop_in (pure version) ---

    #[rstest]
    #[case("containerd", Some("containerd://2.2.0"), true)]
    #[case("containerd", Some("containerd://2.0.0"), true)]
    #[case("containerd", Some("containerd://1.7.0"), false)]
    #[case("containerd", None, false)]
    #[case("crio", Some("containerd://2.2.0"), false)]
    #[case("crio", None, false)]
    #[case("k0s-worker", None, true)]
    #[case("k0s-controller", None, true)]
    fn test_is_containerd_capable_of_drop_in(
        #[case] runtime: &str,
        #[case] version: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_containerd_capable_of_drop_in(runtime, version),
            expected,
            "runtime: {}, version: {:?}",
            runtime,
            version
        );
    }

    // --- containerd_version_is_2_2_or_newer ---

    #[rstest]
    #[case("containerd://2.2.0", true)]
    #[case("containerd://2.2.0-rc.1", true)]
    #[case("containerd://2.2.1", true)]
    #[case("containerd://2.3.0", true)]
    #[case("containerd://3.0.0", true)]
    #[case("containerd://2.2.2-bd1.34", true)]
    #[case("containerd://2.0.0", false)]
    #[case("containerd://2.1.5", false)]
    #[case("containerd://2.1.0", false)]
    #[case("containerd://1.7.15", false)]
    #[case("containerd://1.6.28", false)]
    #[case("containerd://", false)]
    #[case("2.2.0", true)]
    #[case("2.2.1", true)]
    #[case("2.3.0", true)]
    #[case("2.1.0", false)]
    #[case("1.7.0", false)]
    #[case("not-a-version", false)]
    fn test_containerd_version_is_2_2_or_newer(#[case] version: &str, #[case] expected: bool) {
        assert_eq!(
            containerd_version_is_2_2_or_newer(version),
            expected,
            "version: {}",
            version
        );
    }
}
