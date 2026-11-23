// Copyright (c) 2026 NVIDIA Corporation
// SPDX-License-Identifier: Apache-2.0

use std::process::Command;

#[test]
fn start_reports_failure_to_the_caller() {
    let bundle = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_containerd-shim-kata-v2"))
        .arg("start")
        .current_dir(bundle.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("shim start"));
}
