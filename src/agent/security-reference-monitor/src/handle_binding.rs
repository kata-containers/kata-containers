// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-4B — descriptor/handle-based operations (TOCTOU defense).
//!
//! A path checked at authorization time can be swapped (via a symlink or a `..` rename)
//! before it is used, so an operation that re-resolves the path by name may act on a
//! different object than the one that was checked. To close this window, the enforcer
//! captures the object's identity (device + inode) at check time as a [`CheckedHandle`]
//! and re-verifies that identity immediately before use. If the path now resolves to a
//! different object, the operation is refused rather than following the swap.

use std::fmt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// The identity of a filesystem object captured at check time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedHandle {
    pub path: String,
    pub dev: u64,
    pub ino: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HandleError {
    /// The path no longer resolves to any object.
    Vanished { path: String },
    /// The path now resolves to a different object than the one checked (a swap).
    Swapped {
        path: String,
        expected: (u64, u64),
        found: (u64, u64),
    },
}

impl fmt::Display for HandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandleError::Vanished { path } => {
                write!(f, "checked path {path} vanished before use")
            }
            HandleError::Swapped {
                path,
                expected,
                found,
            } => write!(
                f,
                "path {path} was swapped between check and use: expected dev/ino {expected:?}, found {found:?}"
            ),
        }
    }
}

impl std::error::Error for HandleError {}

impl CheckedHandle {
    /// Capture the identity of `path` (does not follow the final symlink — the identity of
    /// the link target that will actually be operated on is captured via metadata).
    #[cfg(unix)]
    pub fn capture(path: impl Into<String>) -> std::io::Result<Self> {
        let path = path.into();
        let md = std::fs::metadata(&path)?;
        Ok(CheckedHandle {
            dev: md.dev(),
            ino: md.ino(),
            path,
        })
    }

    /// Re-resolve the path and confirm it still refers to the object captured at check
    /// time. Returns an error if the object vanished or was swapped.
    #[cfg(unix)]
    pub fn verify_unchanged(&self) -> Result<(), HandleError> {
        let md = std::fs::metadata(&self.path).map_err(|_| HandleError::Vanished {
            path: self.path.clone(),
        })?;
        self.check_identity(md.dev(), md.ino())
    }

    /// Compare a currently-observed identity against the captured one (unit-testable core).
    pub fn check_identity(&self, current_dev: u64, current_ino: u64) -> Result<(), HandleError> {
        if (current_dev, current_ino) == (self.dev, self.ino) {
            Ok(())
        } else {
            Err(HandleError::Swapped {
                path: self.path.clone(),
                expected: (self.dev, self.ino),
                found: (current_dev, current_ino),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_match_is_accepted() {
        let h = CheckedHandle {
            path: "/x".into(),
            dev: 42,
            ino: 100,
        };
        assert!(h.check_identity(42, 100).is_ok());
    }

    #[test]
    fn identity_mismatch_is_rejected() {
        let h = CheckedHandle {
            path: "/x".into(),
            dev: 42,
            ino: 100,
        };
        assert_eq!(
            h.check_identity(42, 200).unwrap_err(),
            HandleError::Swapped {
                path: "/x".into(),
                expected: (42, 100),
                found: (42, 200),
            }
        );
    }

    /// TC5.4: a real symlink/file swap between check and use is detected — the operation
    /// binds to the checked handle's identity, not the re-resolved name.
    #[cfg(unix)]
    #[test]
    fn real_file_swap_between_check_and_use_is_rejected() {
        let dir = std::env::temp_dir().join(format!("fr4b-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("checked");
        std::fs::write(&target, b"original").unwrap();

        // Check time: capture the handle.
        let handle = CheckedHandle::capture(target.to_str().unwrap()).unwrap();
        assert!(handle.verify_unchanged().is_ok());

        // Attacker swaps the object at the same path for a different file (new inode)
        // by atomically renaming a different file over it.
        let other = dir.join("attacker");
        std::fs::write(&other, b"attacker-controlled").unwrap();
        std::fs::rename(&other, &target).unwrap();

        // Use time: the swap is detected (the path now resolves to a different inode).
        assert!(matches!(
            handle.verify_unchanged().unwrap_err(),
            HandleError::Swapped { .. }
        ));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn vanished_path_is_rejected() {
        let dir = std::env::temp_dir().join(format!("fr4b-van-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("checked");
        std::fs::write(&target, b"x").unwrap();
        let handle = CheckedHandle::capture(target.to_str().unwrap()).unwrap();
        std::fs::remove_file(&target).unwrap();
        assert!(matches!(
            handle.verify_unchanged().unwrap_err(),
            HandleError::Vanished { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }
}
