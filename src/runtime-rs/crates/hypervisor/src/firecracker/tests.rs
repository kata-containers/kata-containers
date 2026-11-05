// Copyright (c) 2026 Kata Contributors
// SPDX-License-Identifier: Apache-2.0
use super::{process, FcInner};
use std::path::Path;
use tokio::sync::mpsc;

#[test]
fn versions() {
    for version in [
        "Firecracker v0.21.1",
        "Firecracker v1.17.0\nRevision: test",
        "Firecracker 1.2.3",
    ] {
        assert!(process::parse_version(version).is_ok(), "{}", version);
    }
    for version in [
        "Firecracker v0.21.0",
        "Firecracker v0.21.1-rc.1",
        "",
        "Firecracker",
        "Firecracker nope",
        "Firecracker vv1.2.3",
        "other 1.2.3",
    ] {
        assert!(process::parse_version(version).is_err(), "{}", version);
    }
}

#[tokio::test]
async fn configured_binary_version_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut fc = FcInner::new(mpsc::channel(1).0);
    fc.config.path = dir.path().join("missing").to_string_lossy().into_owned();
    assert!(format!("{:#}", fc.validate_version().await.unwrap_err())
        .contains("read Firecracker version"));
    // Use immutable executables: another concurrent process spawn can briefly
    // inherit a newly written file descriptor and make exec fail with ETXTBSY.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/firecracker/testdata");
    fc.config.path = fixtures
        .join("version-valid.sh")
        .to_string_lossy()
        .into_owned();
    fc.validate_version().await.unwrap();
    fc.config.path = fixtures
        .join("version-failed.sh")
        .to_string_lossy()
        .into_owned();
    assert!(fc
        .validate_version()
        .await
        .unwrap_err()
        .to_string()
        .contains("failed"));
    fc.config.path = fixtures
        .join("version-old.sh")
        .to_string_lossy()
        .into_owned();
    assert!(fc
        .validate_version()
        .await
        .unwrap_err()
        .to_string()
        .contains("minimum"));
}
