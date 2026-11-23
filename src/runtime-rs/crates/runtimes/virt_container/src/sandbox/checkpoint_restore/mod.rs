// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use async_trait::async_trait;
use common::error::Error as CommonError;
use common::{
    types::{
        CheckpointSandboxRequest, PrepareRestoreOptions, RestoreSandboxInfo, RestoreSandboxRequest,
    },
    SandboxCheckpointRestore,
};
use hypervisor;

use crate::sandbox::VirtSandbox;

mod artifacts;
mod checkpoint;
mod identity;
mod manifest;
mod restore;
mod sparse;
mod startup;
mod validation;

use manifest::{CheckpointManifest, WORKLOAD_CHECKPOINT_MANIFEST, WORKLOAD_CHECKPOINT_STATE};

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

fn publish_manifest(manifest: &CheckpointManifest, path: &Path) -> Result<()> {
    let partial = path.with_extension("json.partial");
    let result = (|| -> Result<()> {
        fs::write(&partial, serde_json::to_vec_pretty(manifest)?)?;
        fs::File::open(&partial)?.sync_all()?;
        fs::rename(&partial, path)?;
        fs::File::open(path.parent().unwrap())?.sync_all()?;

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&partial);
        let _ = fs::remove_file(path);
    }

    result
}

pub(super) struct RestoreBoot {
    pub(super) options: hypervisor::VmRestoreOptions,
    pub(super) blocks: Vec<hypervisor::BlockDeviceModern>,
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
        self.validate_checkpoint_request(req).await?;
        self.validate_checkpoint_devices(req).await.map(|_| ())
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
