// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::k8s;
use crate::utils;
use anyhow::{Context, Result};
use regex::Regex;

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

pub async fn get_container_runtime(config: &Config) -> Result<String> {
    let runtime_version = k8s::get_node_field(config, ".status.nodeInfo.containerRuntimeVersion")
        .await
        .context("Failed to get container runtime version")?;

    let microk8s = k8s::get_node_field(config, r".metadata.labels.microk8s\.io/cluster")
        .await
        .ok();
    if microk8s.as_deref() == Some("true") {
        return Ok("microk8s".to_string());
    }

    if runtime_version.contains("cri-o") || runtime_version.contains("crio") {
        return Ok("crio".to_string());
    }

    // Detect k3s/rke2 via systemd services rather than the containerd version
    // string, which no longer reliably contains "k3s" in newer releases
    // (e.g. "containerd://2.2.2-bd1.34").
    if utils::host_systemctl(&["is-active", "--quiet", "rke2-agent"]).is_ok() {
        return Ok("rke2-agent".to_string());
    }
    if utils::host_systemctl(&["is-active", "--quiet", "rke2-server"]).is_ok() {
        return Ok("rke2-server".to_string());
    }
    if utils::host_systemctl(&["is-active", "--quiet", "k3s-agent"]).is_ok() {
        return Ok("k3s-agent".to_string());
    }
    if utils::host_systemctl(&["is-active", "--quiet", "k3s"]).is_ok() {
        return Ok("k3s".to_string());
    }

    if utils::host_systemctl(&["is-active", "--quiet", "k0scontroller"]).is_ok() {
        return Ok("k0s-controller".to_string());
    }
    if utils::host_systemctl(&["is-active", "--quiet", "k0sworker"]).is_ok() {
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

/// Check if a containerd version string supports drop-in files.
/// Wrapper around containerd_version_is_2_or_newer for call sites that need Result.
fn check_containerd_version_supports_drop_in(runtime_version: &str) -> Result<()> {
    if containerd_version_is_2_or_newer(runtime_version) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "containerd version does not support drop-in files (requires >= 2.0), got '{}'",
            runtime_version
        ))
    }
}

pub async fn is_containerd_capable_of_using_drop_in_files(
    config: &Config,
    runtime: &str,
) -> Result<bool> {
    if RUNTIMES_WITHOUT_CONTAINERD_DROP_IN_SUPPORT.contains(&runtime) {
        return Ok(false);
    }

    // k0s always supports drop-in files (auto-loads from containerd.d/)
    if runtime == "k0s-worker" || runtime == "k0s-controller" {
        return Ok(true);
    }

    // Check containerd version - only 2.0+ supports drop-in files properly
    let runtime_version =
        k8s::get_node_field(config, ".status.nodeInfo.containerRuntimeVersion").await?;

    Ok(check_containerd_version_supports_drop_in(&runtime_version).is_ok())
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

/// Restart the CRI runtime and wait for the node to become ready.
pub async fn restart_and_wait_for_ready(config: &Config, runtime: &str) -> Result<()> {
    log::info!("restart_and_wait_for_ready: Restarting runtime");
    lifecycle::restart_cri_runtime(config, runtime).await?;
    log::info!("restart_and_wait_for_ready: Successfully restarted runtime");

    log::info!("restart_and_wait_for_ready: Waiting for node to become ready (timeout: 300s)");
    lifecycle::wait_till_node_is_ready_timeout(config, Some(300)).await?;
    log::info!("restart_and_wait_for_ready: Node is ready");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

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

    // --- check_containerd_version_supports_drop_in (Result wrapper) ---

    #[rstest]
    #[case("containerd://2.0.0", true)]
    #[case("containerd://2.1.5-k3s1", true)]
    #[case("containerd://1.7.15", false)]
    #[case("containerd://1.6.28", false)]
    #[case("containerd://", false)]
    #[case("1.7.0", false)]
    #[case("not-a-version", false)]
    fn test_check_containerd_version_supports_drop_in(
        #[case] version: &str,
        #[case] expected_ok: bool,
    ) {
        let result = check_containerd_version_supports_drop_in(version);
        assert_eq!(
            result.is_ok(),
            expected_ok,
            "version: {}, result: {:?}",
            version,
            result
        );
    }
}
