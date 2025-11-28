// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use std::process::Command;

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

/// Get kata containers config path based on shim type
pub fn get_kata_containers_config_path(shim: &str, dest_dir: &str) -> String {
    let golang_config_path = format!("{dest_dir}/share/defaults/kata-containers");
    let rust_config_path = format!("{golang_config_path}/runtime-rs");

    match shim {
        "cloud-hypervisor"
        | "dragonball"
        | "qemu-runtime-rs"
        | "qemu-coco-dev-runtime-rs"
        | "qemu-se-runtime-rs" => rust_config_path,
        _ => golang_config_path,
    }
}

/// Get kata containers runtime path based on shim type
pub fn get_kata_containers_runtime_path(shim: &str, dest_dir: &str) -> String {
    match shim {
        "cloud-hypervisor"
        | "dragonball"
        | "qemu-runtime-rs"
        | "qemu-coco-dev-runtime-rs"
        | "qemu-se-runtime-rs" => {
            format!("{dest_dir}/runtime-rs/bin/containerd-shim-kata-v2")
        }
        _ => format!("{dest_dir}/bin/containerd-shim-kata-v2"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_kata_containers_config_path_golang() {
        let dest_dir = "/opt/kata";

        // Test Go runtime shims
        assert_eq!(
            get_kata_containers_config_path("qemu", dest_dir),
            "/opt/kata/share/defaults/kata-containers"
        );
        assert_eq!(
            get_kata_containers_config_path("qemu-tdx", dest_dir),
            "/opt/kata/share/defaults/kata-containers"
        );
        assert_eq!(
            get_kata_containers_config_path("qemu-snp", dest_dir),
            "/opt/kata/share/defaults/kata-containers"
        );
        assert_eq!(
            get_kata_containers_config_path("fc", dest_dir),
            "/opt/kata/share/defaults/kata-containers"
        );
    }

    #[test]
    fn test_get_kata_containers_config_path_rust() {
        let dest_dir = "/opt/kata";

        // Test Rust runtime shims
        assert_eq!(
            get_kata_containers_config_path("cloud-hypervisor", dest_dir),
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
        assert_eq!(
            get_kata_containers_config_path("dragonball", dest_dir),
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
        assert_eq!(
            get_kata_containers_config_path("qemu-runtime-rs", dest_dir),
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
        assert_eq!(
            get_kata_containers_config_path("qemu-coco-dev-runtime-rs", dest_dir),
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
        assert_eq!(
            get_kata_containers_config_path("qemu-se-runtime-rs", dest_dir),
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
    }

    #[test]
    fn test_get_kata_containers_config_path_custom_dest() {
        let dest_dir = "/usr/local/kata";

        assert_eq!(
            get_kata_containers_config_path("qemu", dest_dir),
            "/usr/local/kata/share/defaults/kata-containers"
        );
        assert_eq!(
            get_kata_containers_config_path("cloud-hypervisor", dest_dir),
            "/usr/local/kata/share/defaults/kata-containers/runtime-rs"
        );
    }

    #[test]
    fn test_get_kata_containers_runtime_path_golang() {
        let dest_dir = "/opt/kata";

        // Test Go runtime paths
        assert_eq!(
            get_kata_containers_runtime_path("qemu", dest_dir),
            "/opt/kata/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("qemu-tdx", dest_dir),
            "/opt/kata/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("fc", dest_dir),
            "/opt/kata/bin/containerd-shim-kata-v2"
        );
    }

    #[test]
    fn test_get_kata_containers_runtime_path_rust() {
        let dest_dir = "/opt/kata";

        // Test Rust runtime paths
        assert_eq!(
            get_kata_containers_runtime_path("cloud-hypervisor", dest_dir),
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("dragonball", dest_dir),
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("qemu-runtime-rs", dest_dir),
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("qemu-coco-dev-runtime-rs", dest_dir),
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("qemu-se-runtime-rs", dest_dir),
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );
    }

    #[test]
    fn test_get_kata_containers_runtime_path_custom_dest() {
        let dest_dir = "/custom/path";

        assert_eq!(
            get_kata_containers_runtime_path("qemu", dest_dir),
            "/custom/path/bin/containerd-shim-kata-v2"
        );
        assert_eq!(
            get_kata_containers_runtime_path("cloud-hypervisor", dest_dir),
            "/custom/path/runtime-rs/bin/containerd-shim-kata-v2"
        );
    }

    #[test]
    fn test_binary_paths_opt_kata_bin() {
        // Test all Go runtime shims use /opt/kata/bin
        let dest_dir = "/opt/kata";
        let go_shims = vec![
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
            "stratovirt",
            "clh",
            "remote",
        ];

        for shim in go_shims {
            let path = get_kata_containers_runtime_path(shim, dest_dir);
            assert_eq!(
                path, "/opt/kata/bin/containerd-shim-kata-v2",
                "Failed for shim: {}",
                shim
            );
        }
    }

    #[test]
    fn test_binary_paths_opt_kata_runtime_rs_bin() {
        // Test all Rust runtime shims use /opt/kata/runtime-rs/bin
        let dest_dir = "/opt/kata";
        let rust_shims = vec![
            "cloud-hypervisor",
            "dragonball",
            "qemu-runtime-rs",
            "qemu-coco-dev-runtime-rs",
            "qemu-se-runtime-rs",
        ];

        for shim in rust_shims {
            let path = get_kata_containers_runtime_path(shim, dest_dir);
            assert_eq!(
                path, "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2",
                "Failed for shim: {}",
                shim
            );
        }
    }

    #[test]
    fn test_config_paths_share_defaults() {
        // Test Go runtime config paths use /opt/kata/share/defaults/kata-containers
        let dest_dir = "/opt/kata";
        let go_shims = vec!["qemu", "qemu-tdx", "fc", "clh"];

        for shim in go_shims {
            let path = get_kata_containers_config_path(shim, dest_dir);
            assert_eq!(
                path, "/opt/kata/share/defaults/kata-containers",
                "Failed for shim: {}",
                shim
            );
        }
    }

    #[test]
    fn test_config_paths_runtime_rs() {
        // Test Rust runtime config paths use /opt/kata/share/defaults/kata-containers/runtime-rs
        let dest_dir = "/opt/kata";
        let rust_shims = vec![
            "cloud-hypervisor",
            "dragonball",
            "qemu-runtime-rs",
            "qemu-coco-dev-runtime-rs",
            "qemu-se-runtime-rs",
        ];

        for shim in rust_shims {
            let path = get_kata_containers_config_path(shim, dest_dir);
            assert_eq!(
                path, "/opt/kata/share/defaults/kata-containers/runtime-rs",
                "Failed for shim: {}",
                shim
            );
        }
    }

    #[test]
    fn test_full_deployment_paths_go_runtime() {
        // Test complete deployment structure for Go runtime
        let dest_dir = "/opt/kata";
        let shim = "qemu-tdx";

        let config_path = get_kata_containers_config_path(shim, dest_dir);
        let runtime_path = get_kata_containers_runtime_path(shim, dest_dir);

        // Expected paths for Go runtime
        assert_eq!(config_path, "/opt/kata/share/defaults/kata-containers");
        assert_eq!(runtime_path, "/opt/kata/bin/containerd-shim-kata-v2");

        // Config file would be at
        let config_file = format!("{}/configuration-{}.toml", config_path, shim);
        assert_eq!(
            config_file,
            "/opt/kata/share/defaults/kata-containers/configuration-qemu-tdx.toml"
        );
    }

    #[test]
    fn test_full_deployment_paths_rust_runtime() {
        // Test complete deployment structure for Rust runtime
        let dest_dir = "/opt/kata";
        let shim = "cloud-hypervisor";

        let config_path = get_kata_containers_config_path(shim, dest_dir);
        let runtime_path = get_kata_containers_runtime_path(shim, dest_dir);

        // Expected paths for Rust runtime
        assert_eq!(
            config_path,
            "/opt/kata/share/defaults/kata-containers/runtime-rs"
        );
        assert_eq!(
            runtime_path,
            "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"
        );

        // Config file would be at
        let config_file = format!("{}/configuration-{}.toml", config_path, shim);
        assert_eq!(
            config_file,
            "/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-cloud-hypervisor.toml"
        );
    }

    #[test]
    fn test_mixed_deployment_both_runtimes() {
        // Test that both Go and Rust runtimes can coexist
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

        // Verify Go runtime paths
        assert!(qemu_config.ends_with("kata-containers"));
        assert!(qemu_binary.ends_with("/bin/containerd-shim-kata-v2"));

        // Verify Rust runtime paths
        assert!(clh_config.ends_with("runtime-rs"));
        assert!(clh_binary.ends_with("/runtime-rs/bin/containerd-shim-kata-v2"));
    }
}
