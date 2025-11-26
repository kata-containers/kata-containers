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

pub async fn get_container_runtime(config: &Config) -> Result<String> {
    let runtime_version = k8s::get_node_field(config, ".status.nodeInfo.containerRuntimeVersion")
        .await
        .context("Failed to get container runtime version")?;

    let microk8s = k8s::get_node_field(config, ".metadata.labels.microk8s\\.io/cluster")
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
/// Returns true if version >= 2.0
fn check_containerd_version_supports_drop_in(runtime_version: &str) -> Result<bool> {
    let version_re = Regex::new(r"containerd://(\d+)\.(\d+)")?;
    if let Some(caps) = version_re.captures(runtime_version) {
        let major: u32 = caps.get(1).unwrap().as_str().parse()?;
        if major < 2 {
            return Ok(false);
        }
    }
    Ok(true)
}

pub async fn is_containerd_capable_of_using_drop_in_files(
    config: &Config,
    runtime: &str,
) -> Result<bool> {
    if matches!(
        runtime,
        "crio" | "k0s-worker" | "k0s-controller" | "microk8s"
    ) {
        return Ok(false);
    }

    // Check containerd version - only 2.0+ supports drop-in files properly
    let runtime_version =
        k8s::get_node_field(config, ".status.nodeInfo.containerRuntimeVersion").await?;

    check_containerd_version_supports_drop_in(&runtime_version)
}

pub async fn configure_cri_runtime(config: &Config, runtime: &str) -> Result<()> {
    match runtime {
        "crio" => {
            crio::configure_crio(config).await?;
        }
        "containerd" | "k3s" | "k3s-agent" | "rke2-agent" | "rke2-server" | "k0s-controller"
        | "k0s-worker" | "microk8s" => {
            containerd::configure_containerd(config, runtime).await?;
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported runtime: {runtime}"));
        }
    }

    Ok(())
}

pub async fn cleanup_cri_runtime(config: &Config, runtime: &str) -> Result<()> {
    log::info!(
        "cleanup_cri_runtime: Starting cleanup for runtime={}",
        runtime
    );
    match runtime {
        "crio" => {
            log::info!("cleanup_cri_runtime: Cleaning up crio");
            crio::cleanup_crio(config).await?;
            log::info!("cleanup_cri_runtime: Successfully cleaned up crio");
        }
        "containerd" | "k3s" | "k3s-agent" | "rke2-agent" | "rke2-server" | "k0s-controller"
        | "k0s-worker" | "microk8s" => {
            log::info!("cleanup_cri_runtime: Cleaning up containerd");
            containerd::cleanup_containerd(config, runtime).await?;
            log::info!("cleanup_cri_runtime: Successfully cleaned up containerd");
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported runtime: {runtime}"));
        }
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

    #[test]
    fn test_containerd_version_1_6_returns_false() {
        let result = check_containerd_version_supports_drop_in("containerd://1.6.28");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_containerd_version_1_7_returns_false() {
        let result = check_containerd_version_supports_drop_in("containerd://1.7.15");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_containerd_version_2_0_returns_true() {
        let result = check_containerd_version_supports_drop_in("containerd://2.0.0");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_2_1_returns_true() {
        let result = check_containerd_version_supports_drop_in("containerd://2.1.5");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_2_2_returns_true() {
        let result = check_containerd_version_supports_drop_in("containerd://2.2.0");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_2_3_returns_true() {
        let result = check_containerd_version_supports_drop_in("containerd://2.3.1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_with_prerelease() {
        let result = check_containerd_version_supports_drop_in("containerd://2.0.0-rc.1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_invalid_format() {
        // Missing version number - should still return Ok(true) as fallback
        let result = check_containerd_version_supports_drop_in("containerd://");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_no_protocol() {
        // No protocol prefix - should still return Ok(true) as fallback
        let result = check_containerd_version_supports_drop_in("1.7.0");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn test_containerd_version_malformed() {
        // Malformed version - should still return Ok(true) as fallback
        let result = check_containerd_version_supports_drop_in("not-a-version");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }
}
