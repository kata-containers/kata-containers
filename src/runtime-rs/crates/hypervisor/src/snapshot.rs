// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Optional VM snapshot and restore support for hypervisors.
//!
//! A hypervisor opts in by implementing [`VmSnapshot`] and returning `Some(self)`
//! from [`crate::Hypervisor::snapshot`]. Callers that need snapshots (the VM
//! factory, for instance) ask for the capability instead of assuming it.
//!
//! All artifacts for one snapshot live under a single directory. The layout
//! inside that directory is owned by the hypervisor that wrote it.

use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;

/// Parameters for restoring a VM from a snapshot directory.
#[derive(Debug)]
pub struct RestoreVmRequest {
    /// Directory holding the artifacts written by [`VmSnapshot::save_vm`].
    pub snapshot_dir: PathBuf,
    /// Seconds to wait for the VMM to come up and complete the restore.
    pub timeout_secs: i32,
}

#[async_trait]
pub trait VmSnapshot: Send + Sync {
    /// Arrange, before the source VM boots, whatever the backend needs so a
    /// later [`VmSnapshot::save_vm`] into `snapshot_dir` can succeed.
    /// Dragonball, for example, must not let a template capture an
    /// established upcall connection.
    ///
    /// Backends that can snapshot a normally booted VM need not implement this.
    async fn prepare_template_source(&self, _snapshot_dir: &Path) -> Result<()> {
        Ok(())
    }

    /// Write the VM state into `snapshot_dir`. The caller must have paused the
    /// VM first.
    async fn save_vm(&self, snapshot_dir: &Path) -> Result<()>;

    /// Bring up the VMM and restore the VM from the snapshot, instead of
    /// [`crate::Hypervisor::start_vm`]. The VM is left **paused**; the caller
    /// resumes it with [`crate::Hypervisor::resume_vm`].
    async fn restore_vm(&self, request: RestoreVmRequest) -> Result<()>;

    /// Check that `snapshot_dir` holds a complete snapshot for this backend.
    fn validate_snapshot(&self, snapshot_dir: &Path) -> Result<()>;
}

/// Return an error naming the first missing artifact under `snapshot_dir`.
pub(crate) fn require_snapshot_files(snapshot_dir: &Path, files: &[&str]) -> Result<()> {
    for file in files {
        let path = snapshot_dir.join(file);
        std::fs::metadata(&path)
            .map_err(|error| anyhow::anyhow!("snapshot artifact {}: {}", path.display(), error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_snapshot_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("state"), b"").unwrap();

        require_snapshot_files(dir.path(), &["state"]).unwrap();

        let error = require_snapshot_files(dir.path(), &["state", "memory"]).unwrap_err();
        assert!(error.to_string().contains("memory"));
    }
}
