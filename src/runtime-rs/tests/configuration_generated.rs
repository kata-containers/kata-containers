// Copyright (c) 2026 Kata Containers contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "kata-runtime-rs-config-test-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_firmware(config: &Path, expected: &str) {
    let content = fs::read_to_string(config).unwrap();
    let actual = content
        .lines()
        .find(|line| line.starts_with("firmware = "))
        .unwrap_or_else(|| panic!("missing firmware setting in {}", config.display()));

    assert_eq!(
        actual,
        format!("firmware = \"{expected}\""),
        "{}",
        config.display()
    );
}

fn generate_configs(runtime_dir: &Path, output_dir: &Path, arch: &str) {
    fs::create_dir_all(output_dir).unwrap();

    let config_names = [
        "configuration-clh-runtime-rs.toml",
        "configuration-clh-azure-runtime-rs.toml",
        "configuration-qemu-runtime-rs.toml",
        "configuration-qemu-coco-dev-runtime-rs.toml",
        "configuration-dragonball.toml",
        "configuration-openvmm-azure-runtime-rs.toml",
        "configuration-remote.toml",
    ];
    for config_name in config_names {
        fs::copy(
            runtime_dir.join("config").join(format!("{config_name}.in")),
            output_dir.join(format!("{config_name}.in")),
        )
        .unwrap();
    }

    let clh = output_dir.join(config_names[0]);
    let clh_azure = output_dir.join(config_names[1]);
    let qemu = output_dir.join(config_names[2]);
    let qemu_coco = output_dir.join(config_names[3]);
    let dragonball = output_dir.join(config_names[4]);
    let openvmm = output_dir.join(config_names[5]);
    let remote = output_dir.join(config_names[6]);
    let output = Command::new("make")
        .current_dir(runtime_dir)
        .arg(format!("ARCH={arch}"))
        // aarch64-options.mk does not set CLHCMD or REMOTE, so those configs
        // are omitted from CONFIGS unless the names are provided here.
        .arg("CLHCMD=cloud-hypervisor")
        .arg("REMOTE=remote")
        .arg(format!("CONFIG_CLH={}", clh.display()))
        .arg(format!("CONFIG_CLH_AZURE={}", clh_azure.display()))
        .arg(format!("CONFIG_QEMU={}", qemu.display()))
        .arg(format!("CONFIG_QEMU_COCO_DEV={}", qemu_coco.display()))
        .arg(format!("CONFIG_DB={}", dragonball.display()))
        .arg(format!("CONFIG_OPENVMM={}", openvmm.display()))
        .arg(format!("CONFIG_REMOTE={}", remote.display()))
        .args([
            &clh,
            &clh_azure,
            &qemu,
            &qemu_coco,
            &dragonball,
            &openvmm,
            &remote,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "make failed for {arch}:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_non_qemu_configs_do_not_inherit_qemu_firmware() {
    let runtime_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = TempDir::new();
    let empty_firmware_configs = [
        "configuration-clh-runtime-rs.toml",
        "configuration-clh-azure-runtime-rs.toml",
        "configuration-dragonball.toml",
        "configuration-openvmm-azure-runtime-rs.toml",
        "configuration-remote.toml",
    ];
    let qemu_firmware_configs = [
        "configuration-qemu-runtime-rs.toml",
        "configuration-qemu-coco-dev-runtime-rs.toml",
    ];

    for arch in ["x86_64", "aarch64"] {
        let output_dir = temp_dir.0.join(arch);
        generate_configs(runtime_dir, &output_dir, arch);
        for config_name in empty_firmware_configs {
            assert_firmware(&output_dir.join(config_name), "");
        }
    }

    for config_name in qemu_firmware_configs {
        assert_firmware(&temp_dir.0.join("x86_64").join(config_name), "");
        assert_firmware(
            &temp_dir.0.join("aarch64").join(config_name),
            "/usr/share/aavmf/AAVMF_CODE.fd",
        );
    }
}
