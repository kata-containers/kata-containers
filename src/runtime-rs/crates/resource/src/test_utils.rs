// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Real-mount tests are ignored by default because they need root; run with `--ignored`.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use hypervisor::{device::device_manager::DeviceManager, qemu::Qemu};
use kata_types::config::hypervisor::SharedFsInfo;
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use tokio::sync::RwLock;

use crate::share_fs::{self, get_host_rw_shared_path, ShareFs};

const CHILD_ENV: &str = "KATA_RESOURCE_MOUNT_TEST_CHILD";

/// Isolate mounts so test failures cannot affect the host or other tests.
pub(crate) fn in_private_mount_namespace(test: &str) -> bool {
    if std::env::var_os(CHILD_ENV).is_some() {
        return true;
    }
    let output = Command::new("unshare")
        .args(["--mount", "--propagation", "private", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", test, "--ignored", "--nocapture"])
        .env(CHILD_ENV, "1")
        .output()
        .expect("start private mount namespace (requires unshare)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{test} failed in its mount namespace:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    false
}

/// Shared propagation reproduces mounts appearing in the read-only sandbox export.
/// Detach before temporary-directory cleanup to protect mounted data after test failures.
pub(crate) struct SharedTmpfs {
    dir: tempfile::TempDir,
}

impl SharedTmpfs {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        mount(
            Some("tmpfs"),
            dir.path(),
            Some("tmpfs"),
            MsFlags::empty(),
            None::<&str>,
        )
        .unwrap();
        mount(
            None::<&str>,
            dir.path(),
            None::<&str>,
            MsFlags::MS_SHARED,
            None::<&str>,
        )
        .unwrap();
        Self { dir }
    }

    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }

    pub(crate) fn sandbox(&self) -> String {
        let sid = self.path().join("sandbox").to_str().unwrap().to_owned();
        let rw = get_host_rw_shared_path(&sid);
        let ro = rw.with_file_name("ro");
        fs::create_dir_all(&rw).unwrap();
        fs::create_dir_all(&ro).unwrap();
        kata_sys_util::mount::bind_mount_unchecked(&rw, &ro, true, MsFlags::MS_SLAVE).unwrap();
        sid
    }
}

impl Drop for SharedTmpfs {
    fn drop(&mut self) {
        let _ = umount2(self.dir.path(), MntFlags::MNT_DETACH);
    }
}

/// Inject a directory-removal failure without preventing mount operations.
pub(crate) fn set_immutable(path: impl AsRef<Path>, immutable: bool) {
    let status = Command::new("chattr")
        .arg(if immutable { "+i" } else { "-i" })
        .arg(path.as_ref())
        .status()
        .expect("run chattr");
    assert!(status.success(), "chattr failed on {:?}", path.as_ref());
}

pub(crate) fn inline_share_fs(sid: &str) -> Arc<dyn ShareFs> {
    let config = SharedFsInfo {
        shared_fs: Some("inline-virtio-fs".to_owned()),
        ..Default::default()
    };
    share_fs::new(sid, &config).unwrap().share_fs
}

pub(crate) async fn device_manager() -> RwLock<DeviceManager> {
    RwLock::new(
        DeviceManager::new(Arc::new(Qemu::new()), None)
            .await
            .unwrap(),
    )
}

pub(crate) fn is_mounted(path: impl AsRef<Path>) -> bool {
    kata_sys_util::mount::get_linux_mount_info(path.as_ref().to_str().unwrap()).is_ok()
}
