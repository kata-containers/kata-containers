// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use common::error::Error as CommonError;
use common::{
    types::{
        CheckpointSandboxRequest, PrepareRestoreOptions, RestoreSandboxInfo, RestoreSandboxRequest,
    },
    SandboxCheckpointRestore,
};
use ring::digest::{Context as Sha256Context, SHA256};

use crate::sandbox::VirtSandbox;

mod artifacts;
mod checkpoint;
mod identity;
mod manifest;
mod restore;
mod sparse;
mod validation;

use manifest::{CheckpointManifest, WORKLOAD_CHECKPOINT_MANIFEST, WORKLOAD_CHECKPOINT_STATE};
use restore::RestoreBoot;
pub(super) use validation::RestoreInventoryValidation;
use validation::{CheckpointInventoryValidation, CheckpointRequestValidation};

pub(super) fn file_integrity(path: &Path) -> Result<(u64, String)> {
    let file = fs::File::open(path)
        .with_context(|| format!("open file for integrity check: {}", path.display()))?;
    let size = file
        .metadata()
        .with_context(|| format!("inspect file for integrity check: {}", path.display()))?
        .len();

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256Context::new(&SHA256);

    // This helper runs from Tokio worker tasks whose stacks are small.
    // Keep the large I/O buffer on the heap to avoid aborting a deeply
    // nested Restore call with a worker-stack overflow.
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .with_context(|| format!("read file for integrity check: {}", path.display()))?;
        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    let digest = hasher.finish();
    let sha256 = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok((size, sha256))
}

fn checkpoint_files(path: &str) -> Result<(PathBuf, PathBuf)> {
    let root = Path::new(path);
    if !root.is_absolute() {
        return Err(CommonError::InvalidSandboxOperation(
            "checkpoint path must be absolute".to_string(),
        )
        .into());
    }

    let metadata = fs::symlink_metadata(root).map_err(|err| {
        CommonError::InvalidSandboxOperation(format!(
            "cannot inspect checkpoint directory {}: {err}",
            root.display()
        ))
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CommonError::InvalidSandboxOperation(
            "checkpoint path must be a non-symlink directory".to_string(),
        )
        .into());
    }

    Ok((
        root.join(WORKLOAD_CHECKPOINT_STATE),
        root.join(WORKLOAD_CHECKPOINT_MANIFEST),
    ))
}

pub(super) struct PendingRestore {
    boot: Option<RestoreBoot>,
    checkpoint_path: String,
    options: HashMap<String, String>,
    manifest: CheckpointManifest,
}

#[derive(Default)]
pub(super) struct RestoreState {
    prepare: Option<PrepareRestoreOptions>,
    pending: Option<PendingRestore>,
    artifact_dir: Option<PathBuf>,
}

impl RestoreState {
    pub(super) fn is_restoring(&self) -> bool {
        self.prepare.is_some() || self.pending.is_some()
    }

    pub(super) fn artifact_dir(&self) -> Option<PathBuf> {
        self.artifact_dir.clone()
    }

    pub(super) fn take_boot(&mut self) -> Option<RestoreBoot> {
        self.pending.as_mut()?.boot.take()
    }

    pub(super) fn set_artifact_dir(&mut self, path: PathBuf) {
        self.artifact_dir = Some(path);
    }

    fn set_pending(&mut self, pending: PendingRestore) {
        self.pending = Some(pending);
    }

    fn set_prepare(&mut self, prepare: PrepareRestoreOptions) {
        self.prepare = Some(prepare);
    }

    fn take_prepare(&mut self) -> Option<PrepareRestoreOptions> {
        self.prepare.take()
    }

    fn take_pending(&mut self) -> Option<PendingRestore> {
        self.pending.take()
    }

    pub(super) fn reset(&mut self) {
        self.prepare = None;
        self.pending = None;
        self.artifact_dir = None;
    }
}

#[async_trait]
impl SandboxCheckpointRestore for VirtSandbox {
    async fn prepare_restore_sandbox(&self, options: &PrepareRestoreOptions) -> Result<()> {
        self.prepare_restore_request(options).await
    }

    async fn validate_checkpoint_sandbox(&self, req: &CheckpointSandboxRequest) -> Result<()> {
        CheckpointRequestValidation {
            sandbox_id: &self.sid,
            request: req,
            resources: self.resource_manager.as_ref(),
        }
        .validate()
        .await?;

        CheckpointInventoryValidation {
            request: req,
            resources: self.resource_manager.as_ref(),
        }
        .validate()
        .await
        .map(|_| ())
    }

    async fn checkpoint_sandbox(
        &self,
        req: &CheckpointSandboxRequest,
        owns_output: &mut bool,
    ) -> Result<()> {
        self.checkpoint_workload(req, owns_output).await
    }

    async fn cleanup_sandbox_checkpoint(&self, req: &CheckpointSandboxRequest) -> Result<()> {
        self.cleanup_checkpoint(req).await
    }

    async fn restore_sandbox(&self, req: &RestoreSandboxRequest) -> Result<RestoreSandboxInfo> {
        self.restore_workload(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_error_keeps_file_path_and_io_cause() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-artifact");
        let err = file_integrity(&missing).unwrap_err();
        assert!(err.to_string().contains(missing.to_str().unwrap()));
        assert_eq!(
            err.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}
