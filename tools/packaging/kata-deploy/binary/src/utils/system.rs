// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use std::path::{Path, PathBuf};

pub const RUST_SHIMS: &[&str] = &[
    "clh-azure-runtime-rs",
    "clh-runtime-rs",
    "dragonball",
    "openvmm-azure-runtime-rs",
    "qemu-runtime-rs",
    "qemu-nvidia-cpu-runtime-rs",
    "qemu-nvidia-gpu-runtime-rs",
    "qemu-nvidia-gpu-snp-runtime-rs",
    "qemu-nvidia-gpu-tdx-runtime-rs",
    "qemu-coco-dev-runtime-rs",
    "qemu-se-runtime-rs",
    "qemu-snp-runtime-rs",
    "qemu-tdx-runtime-rs",
];

/// Host binary directories mounted read-only into the container (see Helm chart).
///
/// kata-deploy cannot *run* the binaries it finds there: they are linked
/// against a dynamic loader and libraries that only exist in the host's mount
/// namespace, which the container no longer has access to. It can inspect
/// them, which is enough to tell what a host tool supports.
///
/// Ordered as the host's own PATH is, so that what is found here is what the
/// host would run. A node can hold several versions of the same tool.
const HOST_BIN_DIRS: &[&str] = &[
    "/host-usr-local/sbin",
    "/host-usr-local/bin",
    "/host-usr/sbin",
    "/host-usr/bin",
    "/host-sbin",
    "/host-bin",
];

pub fn is_rust_shim(shim: &str) -> bool {
    RUST_SHIMS.contains(&shim)
}

/// Locate a host program among the binary directories mounted into the container.
pub fn find_host_program(program: &str) -> Option<PathBuf> {
    HOST_BIN_DIRS
        .iter()
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.is_file())
}

/// Perform a systemctl-equivalent operation through the host systemd D-Bus API.
pub async fn host_systemctl(args: &[&str]) -> Result<()> {
    super::systemd::systemctl(args).await
}

pub async fn host_unit_active_since(unit: &str) -> Result<Option<std::time::SystemTime>> {
    super::systemd::unit_active_since(unit).await
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
    #[case(
        "clh-runtime-rs",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtime-rs"
    )]
    #[case(
        "qemu-runtime-rs",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtime-rs"
    )]
    #[case("qemu", "/custom/path", "/custom/path/share/defaults/kata-containers")]
    #[case(
        "clh-runtime-rs",
        "/custom/path",
        "/custom/path/share/defaults/kata-containers/runtime-rs"
    )]
    fn test_get_kata_containers_original_config_path(
        #[case] shim: &str,
        #[case] base_dir: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            get_kata_containers_original_config_path(shim, base_dir),
            expected
        );
    }

    // Tests for get_kata_containers_config_path (per-shim runtime directories)
    #[rstest]
    #[case(
        "qemu",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtimes/qemu"
    )]
    #[case(
        "qemu-tdx",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtimes/qemu-tdx"
    )]
    #[case(
        "fc",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtimes/fc"
    )]
    #[case(
        "clh-runtime-rs",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/clh-runtime-rs"
    )]
    #[case(
        "qemu-runtime-rs",
        "/opt/kata",
        "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/qemu-runtime-rs"
    )]
    #[case(
        "qemu",
        "/custom/path",
        "/custom/path/share/defaults/kata-containers/runtimes/qemu"
    )]
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
            &["clh-runtime-rs"],
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
            "qemu-nvidia-cpu",
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
        assert_eq!(
            config_path,
            "/opt/kata/share/defaults/kata-containers/runtimes/qemu-tdx"
        );
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
        let shim = "clh-runtime-rs";

        let config_path = get_kata_containers_config_path(shim, dest_dir);
        let original_path = get_kata_containers_original_config_path(shim, dest_dir);
        let runtime_path = get_kata_containers_runtime_path(shim, dest_dir);

        // Expected paths for Rust runtime with per-shim directory
        assert_eq!(
            config_path,
            "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/clh-runtime-rs"
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
            "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/clh-runtime-rs/configuration-clh-runtime-rs.toml"
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
        let clh_config = get_kata_containers_config_path("clh-runtime-rs", dest_dir);
        let clh_binary = get_kata_containers_runtime_path("clh-runtime-rs", dest_dir);

        // Both should have different paths
        assert_ne!(qemu_config, clh_config);
        assert_ne!(qemu_binary, clh_binary);

        // Verify Go runtime paths include per-shim directory
        assert!(qemu_config.contains("/runtimes/qemu"));
        assert!(qemu_binary.ends_with("/bin/containerd-shim-kata-v2"));

        // Verify Rust runtime paths include per-shim directory
        assert!(clh_config.contains("/runtimes/clh-runtime-rs"));
        assert!(clh_binary.ends_with("/runtime-rs/bin/containerd-shim-kata-v2"));
    }
}
