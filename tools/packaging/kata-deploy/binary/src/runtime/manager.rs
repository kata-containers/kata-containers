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

/// Runtimes that don't support containerd drop-in configuration files
const RUNTIMES_WITHOUT_CONTAINERD_DROP_IN_SUPPORT: &[&str] = &[
    "crio",
    "k0s-worker",
    "k0s-controller",
];

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

    if runtime_version.contains("containerd") && runtime_version.contains("-k3s") {
        // Check systemd services (ignore errors - service might not exist)
        let _ = utils::host_systemctl(&["is-active", "--quiet", "rke2-agent"]);
        if utils::host_systemctl(&["is-active", "--quiet", "rke2-agent"]).is_ok() {
            return Ok("rke2-agent".to_string());
        }
        if utils::host_systemctl(&["is-active", "--quiet", "rke2-server"]).is_ok() {
            return Ok("rke2-server".to_string());
        }
        if utils::host_systemctl(&["is-active", "--quiet", "k3s-agent"]).is_ok() {
            return Ok("k3s-agent".to_string());
        }
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

/// Check if a containerd version string supports drop-in files
/// Returns Ok(()) if version >= 2.0, Err otherwise
fn check_containerd_version_supports_drop_in(runtime_version: &str) -> Result<()> {
    let version_re = Regex::new(r"containerd://(\d+)\.(\d+)")?;
    if let Some(caps) = version_re.captures(runtime_version) {
        let major: u32 = caps.get(1).unwrap().as_str().parse()?;
        if major >= 2 {
            return Ok(());
        }
        return Err(anyhow::anyhow!(
            "containerd version {}.x does not support drop-in files (requires >= 2.0)",
            major
        ));
    }
    // If version string is malformed/unparseable, conservatively assume no support
    Err(anyhow::anyhow!(
        "Unable to parse containerd version from '{}', assuming no drop-in support",
        runtime_version
    ))
}

pub async fn is_containerd_capable_of_using_drop_in_files(
    config: &Config,
    runtime: &str,
) -> Result<bool> {
    if RUNTIMES_WITHOUT_CONTAINERD_DROP_IN_SUPPORT.contains(&runtime) {
        return Ok(false);
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

pub async fn cleanup_cri_runtime(config: &Config, runtime: &str) -> Result<()> {
    log::info!(
        "cleanup_cri_runtime: Starting cleanup for runtime={}",
        runtime
    );
    
    if runtime == "crio" {
        log::info!("cleanup_cri_runtime: Cleaning up crio");
        crio::cleanup_crio(config).await?;
        log::info!("cleanup_cri_runtime: Successfully cleaned up crio");
    } else if is_containerd_based(runtime) {
        log::info!("cleanup_cri_runtime: Cleaning up containerd");
        containerd::cleanup_containerd(config, runtime).await?;
        log::info!("cleanup_cri_runtime: Successfully cleaned up containerd");
    } else {
        return Err(anyhow::anyhow!("Unsupported runtime: {runtime}"));
    }

    if config.helm_post_delete_hook {
        log::info!("cleanup_cri_runtime: Helm post-delete hook, restarting runtime");
        lifecycle::restart_cri_runtime(config, runtime).await?;
        log::info!("cleanup_cri_runtime: Successfully restarted runtime");
    } else {
        log::info!("cleanup_cri_runtime: Not a Helm post-delete hook, skipping runtime restart");
    }

    log::info!("cleanup_cri_runtime: Cleanup completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to test version check with expected error result
    fn assert_version_check_error(version: &str) {
        let result = check_containerd_version_supports_drop_in(version);
        assert!(result.is_err(), "Expected error for version: {}", version);
    }

    /// Helper function to test version check with expected success result
    fn assert_version_check_ok(version: &str) {
        let result = check_containerd_version_supports_drop_in(version);
        assert!(result.is_ok(), "Expected success for version: {}", version);
    }

    #[test]
    fn test_containerd_version_1_6_returns_error() {
        assert_version_check_error("containerd://1.6.28");
    }

    #[test]
    fn test_containerd_version_1_7_returns_error() {
        assert_version_check_error("containerd://1.7.15");
    }

    #[test]
    fn test_containerd_version_2_0_returns_ok() {
        assert_version_check_ok("containerd://2.0.0");
    }

    #[test]
    fn test_containerd_version_2_1_returns_ok() {
        assert_version_check_ok("containerd://2.1.5");
    }

    #[test]
    fn test_containerd_version_2_2_returns_ok() {
        assert_version_check_ok("containerd://2.2.0");
    }

    #[test]
    fn test_containerd_version_2_3_returns_ok() {
        assert_version_check_ok("containerd://2.3.1");
    }

    #[test]
    fn test_containerd_version_with_prerelease() {
        assert_version_check_ok("containerd://2.0.0-rc.1");
    }

    #[test]
    fn test_containerd_version_invalid_format() {
        // Missing version number - conservatively assume no support
        assert_version_check_error("containerd://");
    }

    #[test]
    fn test_containerd_version_no_protocol() {
        // No protocol prefix - conservatively assume no support
        assert_version_check_error("1.7.0");
    }

    #[test]
    fn test_containerd_version_malformed() {
        // Malformed version - conservatively assume no support
        assert_version_check_error("not-a-version");
    }
}
