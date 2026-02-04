// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use std::process::Command;

pub const RUST_SHIMS: &[&str] = &[
    "cloud-hypervisor",
    "dragonball",
    "qemu-runtime-rs",
    "qemu-coco-dev-runtime-rs",
    "qemu-se-runtime-rs",
    "qemu-snp-runtime-rs",
    "qemu-tdx-runtime-rs",
];

pub fn is_rust_shim(shim: &str) -> bool {
    RUST_SHIMS.contains(&shim)
}

/// Execute a command in the host namespace (equivalent to nsenter --target 1 --mount)
pub fn host_exec(command: &[&str]) -> Result<String> {
    // Use nsenter (copied from Alpine) to execute command in host's mount namespace
    // Since we have hostPID: true, PID 1 is the host's init
    let mut nsenter_cmd = vec!["nsenter", "--target", "1", "--mount", "--"];
    nsenter_cmd.extend(command);

    let output = Command::new(nsenter_cmd[0])
        .args(&nsenter_cmd[1..])
        .output()
        .context("Failed to execute command with nsenter")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Command failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Execute systemctl command in host namespace
pub fn host_systemctl(args: &[&str]) -> Result<()> {
    let mut cmd = vec!["systemctl"];
    cmd.extend(args);
    let _output = host_exec(&cmd)?;
    Ok(())
}

/// Get kata containers config path based on shim type.
/// This returns the path where the shim's configuration will be read from.
/// For standard runtimes using drop-in configuration, this is the per-shim directory.
pub fn get_kata_containers_config_path(shim: &str, base_dir: &str) -> String {
    let base_path = get_kata_containers_original_config_path(shim, base_dir);
    format!("{base_path}/runtimes/{shim}")
}

/// Get the original kata containers config path (where configs are installed).
/// This is where the original, unmodified configuration files live.
pub fn get_kata_containers_original_config_path(shim: &str, base_dir: &str) -> String {
    let golang_config_path = format!("{base_dir}/share/defaults/kata-containers");
    let rust_config_path = format!("{golang_config_path}/runtime-rs");

    if is_rust_shim(shim) {
        rust_config_path
    } else {
        golang_config_path
    }
}

/// Get kata containers runtime path based on shim type
pub fn get_kata_containers_runtime_path(shim: &str, base_dir: &str) -> String {
    if is_rust_shim(shim) {
        format!("{base_dir}/runtime-rs/bin/containerd-shim-kata-v2")
    } else {
        format!("{base_dir}/bin/containerd-shim-kata-v2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Helper to test runtime paths for multiple shims expecting the same result
    fn assert_runtime_paths(shims: &[&str], base_dir: &str, expected: &str) {
        for shim in shims {
            assert_eq!(
                get_kata_containers_runtime_path(shim, base_dir),
                expected,
                "Runtime path mismatch for shim '{}'",
                shim
            );
        }
    }

    // Tests for get_kata_containers_original_config_path (where original configs live)
    #[rstest]
    #[case("qemu", "/opt/kata", "/opt/kata/share/defaults/kata-containers")]
    #[case("qemu-tdx", "/opt/kata", "/opt/kata/share/defaults/kata-containers")]
    #[case("fc", "/opt/kata", "/opt/kata/share/defaults/kata-containers")]
    #[case("clh", "/opt/kata", "/opt/kata/share/defaults/kata-containers")]
    #[case("cloud-hypervisor", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtime-rs")]
    #[case("qemu-runtime-rs", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtime-rs")]
    #[case("qemu", "/custom/path", "/custom/path/share/defaults/kata-containers")]
    #[case("cloud-hypervisor", "/custom/path", "/custom/path/share/defaults/kata-containers/runtime-rs")]
    fn test_get_kata_containers_original_config_path(
        #[case] shim: &str,
        #[case] base_dir: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(get_kata_containers_original_config_path(shim, base_dir), expected);
    }

    // Tests for get_kata_containers_config_path (per-shim runtime directories)
    #[rstest]
    #[case("qemu", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtimes/qemu")]
    #[case("qemu-tdx", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtimes/qemu-tdx")]
    #[case("fc", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtimes/fc")]
    #[case("cloud-hypervisor", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/cloud-hypervisor")]
    #[case("qemu-runtime-rs", "/opt/kata", "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/qemu-runtime-rs")]
    #[case("qemu", "/custom/path", "/custom/path/share/defaults/kata-containers/runtimes/qemu")]
    fn test_get_kata_containers_config_path(
        #[case] shim: &str,
        #[case] base_dir: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(get_kata_containers_config_path(shim, base_dir), expected);
    }

    #[test]
    fn test_get_kata_containers_runtime_path_golang() {
        let go_shims = ["qemu", "qemu-tdx", "fc"];
        assert_runtime_paths(
            &go_shims,
            "/opt/kata",
            "/opt/kata/bin/containerd-shim-kata-v2",
        );
    }

    #[test]
    fn test_get_kata_containers_runtime_path_rust() {
        assert_runtime_paths(
            RUST_SHIMS,
            "/opt/kata",
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2",
        );
    }

    #[test]
    fn test_get_kata_containers_runtime_path_custom_dest() {
        assert_runtime_paths(
            &["qemu"],
            "/custom/path",
            "/custom/path/bin/containerd-shim-kata-v2",
        );
        assert_runtime_paths(
            &["cloud-hypervisor"],
            "/custom/path",
            "/custom/path/runtime-rs/bin/containerd-shim-kata-v2",
        );
    }

    #[test]
    fn test_binary_paths_opt_kata_bin() {
        // Test all Go runtime shims use /opt/kata/bin
        let go_shims = [
            "qemu",
            "qemu-tdx",
            "qemu-snp",
            "qemu-se",
            "qemu-coco-dev",
            "qemu-cca",
            "qemu-nvidia-gpu",
            "qemu-nvidia-gpu-tdx",
            "qemu-nvidia-gpu-snp",
            "fc",
            "clh",
            "remote",
        ];
        assert_runtime_paths(
            &go_shims,
            "/opt/kata",
            "/opt/kata/bin/containerd-shim-kata-v2",
        );
    }

    #[test]
    fn test_binary_paths_opt_kata_runtime_rs_bin() {
        // Test all Rust runtime shims use /opt/kata/runtime-rs/bin
        assert_runtime_paths(
            RUST_SHIMS,
            "/opt/kata",
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2",
        );
    }

    #[test]
    fn test_full_deployment_paths_go_runtime() {
        // Test complete deployment structure for Go runtime
        let dest_dir = "/opt/kata";
        let shim = "qemu-tdx";

        let config_path = get_kata_containers_config_path(shim, dest_dir);
        let original_path = get_kata_containers_original_config_path(shim, dest_dir);
        let runtime_path = get_kata_containers_runtime_path(shim, dest_dir);

        // Expected paths for Go runtime with per-shim directory
        assert_eq!(config_path, "/opt/kata/share/defaults/kata-containers/runtimes/qemu-tdx");
        assert_eq!(original_path, "/opt/kata/share/defaults/kata-containers");
        assert_eq!(runtime_path, "/opt/kata/bin/containerd-shim-kata-v2");

        // Config file would be at (symlink to original)
        let config_file = format!("{}/configuration-{}.toml", config_path, shim);
        assert_eq!(
            config_file,
            "/opt/kata/share/defaults/kata-containers/runtimes/qemu-tdx/configuration-qemu-tdx.toml"
        );
    }

    #[test]
    fn test_full_deployment_paths_rust_runtime() {
        // Test complete deployment structure for Rust runtime
        let dest_dir = "/opt/kata";
        let shim = "cloud-hypervisor";

        let config_path = get_kata_containers_config_path(shim, dest_dir);
        let original_path = get_kata_containers_original_config_path(shim, dest_dir);
        let runtime_path = get_kata_containers_runtime_path(shim, dest_dir);

        // Expected paths for Rust runtime with per-shim directory
        assert_eq!(
            config_path,
            "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/cloud-hypervisor"
        );
        assert_eq!(
            original_path,
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
        assert_eq!(
            runtime_path,
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );

        // Config file would be at (symlink to original)
        let config_file = format!("{}/configuration-{}.toml", config_path, shim);
        assert_eq!(
            config_file,
            "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/cloud-hypervisor/configuration-cloud-hypervisor.toml"
        );
    }

    #[test]
    fn test_mixed_deployment_both_runtimes() {
        // Test that both Go and Rust runtimes can coexist with separate directories
        let dest_dir = "/opt/kata";

        // Go runtime
        let qemu_config = get_kata_containers_config_path("qemu", dest_dir);
        let qemu_binary = get_kata_containers_runtime_path("qemu", dest_dir);

        // Rust runtime
        let clh_config = get_kata_containers_config_path("cloud-hypervisor", dest_dir);
        let clh_binary = get_kata_containers_runtime_path("cloud-hypervisor", dest_dir);

        // Both should have different paths
        assert_ne!(qemu_config, clh_config);
        assert_ne!(qemu_binary, clh_binary);

        // Verify Go runtime paths include per-shim directory
        assert!(qemu_config.contains("/runtimes/qemu"));
        assert!(qemu_binary.ends_with("/bin/containerd-shim-kata-v2"));

        // Verify Rust runtime paths include per-shim directory
        assert!(clh_config.contains("/runtimes/cloud-hypervisor"));
        assert!(clh_binary.ends_with("/runtime-rs/bin/containerd-shim-kata-v2"));
    }
}
