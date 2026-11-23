// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use crate::sandbox::VirtSandbox;
use anyhow::Result;
use async_trait::async_trait;
use common::{
    error::Error as CommonError,
    types::{
        CheckpointSandboxRequest, PrepareRestoreOptions, RestoreSandboxInfo, RestoreSandboxRequest,
    },
    SandboxCheckpointRestore,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
mod artifacts;
mod checkpoint;
mod identity;
mod manifest;
mod sparse;
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

#[async_trait]
impl SandboxCheckpointRestore for VirtSandbox {
    async fn prepare_restore_sandbox(&self, _options: &PrepareRestoreOptions) -> Result<()> {
        Err(CommonError::SandboxOperationUnsupported(
            "sandbox restore artifacts are not supported yet".into(),
        )
        .into())
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
    async fn restore_sandbox(&self, _req: &RestoreSandboxRequest) -> Result<RestoreSandboxInfo> {
        Err(CommonError::SandboxOperationUnsupported(
            "sandbox restore artifacts are not supported yet".into(),
        )
        .into())
    }
}
