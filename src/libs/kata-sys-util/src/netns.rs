// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{fs::File, io::ErrorKind, os::unix::io::AsRawFd, path::Path};

use anyhow::{Context, Result};
use nix::errno::Errno;
use nix::mount::{umount2, MntFlags};
use nix::sched::{setns, CloneFlags};
use nix::unistd::{getpid, gettid};
use rand::rng as thread_rng;
use rand::RngExt;

use kata_types::sl;

pub struct NetnsGuard {
    old_netns: Option<File>,
}

impl NetnsGuard {
    pub fn new(new_netns_path: &str) -> Result<Self> {
        let old_netns = if !new_netns_path.is_empty() {
            let current_netns_path = format!("/proc/{}/task/{}/ns/{}", getpid(), gettid(), "net");
            let old_netns = File::open(&current_netns_path)
                .with_context(|| format!("open current netns path {}", current_netns_path))?;
            let new_netns = File::open(new_netns_path)
                .with_context(|| format!("open new netns path {}", new_netns_path))?;
            setns(&new_netns, CloneFlags::CLONE_NEWNET)
                .with_context(|| "set netns to new netns")?;
            info!(
                sl!(),
                "set netns from old {:?} to new {:?} tid {}",
                old_netns,
                new_netns,
                gettid().to_string()
            );
            Some(old_netns)
        } else {
            warn!(sl!(), "skip to set netns for empty netns path");
            None
        };
        Ok(Self { old_netns })
    }
}

impl Drop for NetnsGuard {
    fn drop(&mut self) {
        if let Some(old_netns) = self.old_netns.as_ref() {
            setns(old_netns, CloneFlags::CLONE_NEWNET).unwrap();
            info!(sl!(), "set netns to old {:?}", old_netns.as_raw_fd());
        }
    }
}

// generate the network namespace name
pub fn generate_netns_name() -> String {
    let mut rng = thread_rng();
    let random_bytes: [u8; 16] = rng.random();
    format!(
        "cnitest-{}-{}-{}-{}-{}",
        hex::encode(&random_bytes[..4]),
        hex::encode(&random_bytes[4..6]),
        hex::encode(&random_bytes[6..8]),
        hex::encode(&random_bytes[8..10]),
        hex::encode(&random_bytes[10..])
    )
}

/// Bind-mount the network namespace `netns` (for example /proc/<pid>/ns/net)
/// onto `pin_path`, the way `ip netns` pins namespaces: the namespace stays
/// reachable there after the process it came from is gone. An earlier pin at
/// the path is replaced rather than stacked on; remove_netns() releases it.
pub fn pin_netns(netns: &Path, pin_path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    // The nsfs inode identifies the namespace, and it is taken before
    // anything else: a source that is already gone must not cost whatever
    // pin is there.
    let src =
        std::fs::metadata(netns).with_context(|| format!("stat the netns {}", netns.display()))?;
    // A pin that already holds this namespace is kept: replacing it would
    // leave a moment with no pin at all.
    if is_pinned_netns(pin_path) {
        if let Ok(cur) = std::fs::metadata(pin_path) {
            if (cur.dev(), cur.ino()) == (src.dev(), src.ino()) {
                return Ok(());
            }
        }
    }
    remove_netns(pin_path)?;
    File::create(pin_path)
        .with_context(|| format!("create the netns pin {}", pin_path.display()))?;
    if let Err(e) = nix::mount::mount(
        Some(netns),
        pin_path,
        None::<&str>,
        nix::mount::MsFlags::MS_BIND,
        None::<&str>,
    ) {
        // A plain file is not a pin: nothing must mistake it for a
        // namespace later.
        let _ = std::fs::remove_file(pin_path);
        return Err(e)
            .with_context(|| format!("bind {} onto {}", netns.display(), pin_path.display()));
    }
    Ok(())
}

// from linux.git/include/uapi/linux/magic.h
const NSFS_MAGIC: crate::fs::FsType = 0x6e736673;

/// Whether the path names a process's network namespace through /proc: a
/// reference that holds only while that process lives, never across a shim
/// death, and that pid reuse can point at somebody else entirely.
pub fn is_proc_pid_netns(path: &str) -> bool {
    path.strip_prefix("/proc/")
        .and_then(|rest| rest.strip_suffix("/ns/net"))
        .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether the path holds a pinned network namespace: an nsfs file, the way
/// pin_netns() leaves one. A missing path, or a plain file left by an
/// interrupted pin, is not one.
pub fn is_pinned_netns(path: &Path) -> bool {
    crate::fs::has_fs_magic(path, NSFS_MAGIC)
}

/// Unmount a network namespace and remove its file, ignoring an absent mount or file.
///
/// The caller must verify that Kata owns the namespace before calling this function;
/// ownership is not checked here, and caller-owned namespaces must survive teardown.
pub fn remove_netns(path: &Path) -> Result<()> {
    match umount2(path, MntFlags::MNT_DETACH) {
        Ok(()) | Err(Errno::EINVAL) | Err(Errno::ENOENT) => {}
        Err(e) => return Err(e).with_context(|| format!("unmount netns {}", path.display())),
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            Err(e).with_context(|| format!("remove netns file {}", path.display()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::skip_if_not_root;

    #[test]
    fn test_new_netns_guard() {
        // test run under root
        skip_if_not_root!();

        let new_netns_path = "/proc/1/task/1/ns/net"; // systemd, always exists
        let netns_guard = NetnsGuard::new(new_netns_path).unwrap();
        drop(netns_guard);

        let empty_path = "";
        assert!(NetnsGuard::new(empty_path).unwrap().old_netns.is_none());
    }

    #[test]
    fn test_generate_netns_name() {
        let name1 = generate_netns_name();
        let name2 = generate_netns_name();
        let name3 = generate_netns_name();
        assert_ne!(name1, name2);
        assert_ne!(name2, name3);
        assert_ne!(name1, name3);
    }

    #[test]
    fn test_is_proc_pid_netns() {
        assert!(is_proc_pid_netns("/proc/42/ns/net"));
        assert!(!is_proc_pid_netns("/run/netns/cni-1"));
        assert!(!is_proc_pid_netns("/proc/self/ns/net"));
        assert!(!is_proc_pid_netns("/proc//ns/net"));
    }

    #[test]
    fn test_pin_netns() {
        skip_if_not_root!();

        let dir = tempfile::tempdir().unwrap();
        let pin = dir.path().join("netns-pin");
        assert!(!is_pinned_netns(&pin));
        pin_netns(Path::new("/proc/self/ns/net"), &pin).unwrap();
        assert!(is_pinned_netns(&pin));

        // The pin is a namespace of its own to enter, and a second pin of
        // the same namespace keeps the mount instead of stacking another.
        drop(NetnsGuard::new(pin.to_str().unwrap()).unwrap());
        pin_netns(Path::new("/proc/self/ns/net"), &pin).unwrap();

        // A pin whose source is gone fails without costing the pin that is
        // there.
        pin_netns(Path::new("/proc/0/ns/net"), &pin).unwrap_err();
        assert!(is_pinned_netns(&pin));

        remove_netns(&pin).unwrap();
        assert!(!pin.exists());

        // A failed pin leaves no file behind to be mistaken for a
        // namespace, and a plain file is not a pin.
        pin_netns(Path::new("/proc/0/ns/net"), &pin).unwrap_err();
        assert!(!pin.exists());
        std::fs::File::create(&pin).unwrap();
        assert!(!is_pinned_netns(&pin));
    }

    #[test]
    fn test_remove_netns() {
        skip_if_not_root!();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(generate_netns_name());
        File::create(&path).unwrap();
        nix::mount::mount(
            Some("/proc/self/ns/net"),
            &path,
            None::<&str>,
            nix::mount::MsFlags::MS_BIND,
            None::<&str>,
        )
        .unwrap();

        assert_eq!(
            std::fs::remove_file(&path).unwrap_err().raw_os_error(),
            Some(libc::EBUSY)
        );
        remove_netns(&path).unwrap();
        assert!(!path.exists());
        remove_netns(&path).unwrap();

        // File exists but was never mounted.
        File::create(&path).unwrap();
        remove_netns(&path).unwrap();
        assert!(!path.exists());
    }
}
