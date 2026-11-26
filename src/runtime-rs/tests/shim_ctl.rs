// Copyright (c) 2026 Kata Containers contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::process::Command;

#[test]
fn shim_ctl_reports_missing_oci_spec() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    assert!(!bundle.join("config.json").exists());

    let output = Command::new(env!("CARGO_BIN_EXE_shim-ctl"))
        .current_dir(bundle)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(
        stderr.starts_with("ERROR: ") && stderr.contains("load spec"),
        "stderr: {}",
        stderr
    );
}
