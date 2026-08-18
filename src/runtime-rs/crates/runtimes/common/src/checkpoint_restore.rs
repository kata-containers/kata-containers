// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::types::{
    CheckpointSandboxRequest, RestoreSandboxInfo, RestoreSandboxRequest, RestoredSandboxTask,
    SandboxCheckpointTask, SandboxRestoreTask,
};

pub const RESTORE_PHASE_OPTION: &str = "io.containerd.pod-restore.phase";
pub const RESTORE_PHASE_PREPARE: &str = "prepare";
pub const RESTORE_PHASE_COMPLETE: &str = "complete";

/// Optional Pod-level checkpoint/restore operations supplied by a sandbox runtime.
#[async_trait]
pub trait SandboxCheckpointRestore: Send + Sync {
    async fn validate_checkpoint_sandbox(&self, req: &CheckpointSandboxRequest) -> Result<()>;
    async fn checkpoint_sandbox(&self, req: &CheckpointSandboxRequest) -> Result<()>;
    async fn restore_sandbox(&self, req: &RestoreSandboxRequest) -> Result<RestoreSandboxInfo>;
    async fn prepare_restore_sandbox(
        &self,
        req: &RestoreSandboxRequest,
    ) -> Result<RestoreSandboxInfo>;
    async fn complete_restore_sandbox(
        &self,
        req: &RestoreSandboxRequest,
    ) -> Result<RestoreSandboxInfo>;
    async fn cleanup_sandbox_checkpoint(&self, req: &CheckpointSandboxRequest) -> Result<()>;
}

/// Optional task operations needed to checkpoint and restore an entire sandbox.
#[async_trait]
pub trait ContainerCheckpointRestore: Send + Sync {
    async fn prepare_checkpoint_tasks(&self, tasks: &mut [SandboxCheckpointTask]) -> Result<()>;
    async fn pause_checkpoint_tasks(&self, tasks: &[SandboxCheckpointTask]) -> Result<()>;
    async fn resume_checkpoint_tasks(&self, tasks: &[SandboxCheckpointTask]) -> Result<()>;
    async fn restore_tasks(
        &self,
        tasks: &[SandboxRestoreTask],
        restored: &[RestoredSandboxTask],
    ) -> Result<()>;
}

/// A complete Pod-level checkpoint/restore capability for one runtime instance.
#[derive(Clone)]
pub struct CheckpointRestoreRuntime {
    pub sandbox: Arc<dyn SandboxCheckpointRestore>,
    pub container_manager: Arc<dyn ContainerCheckpointRestore>,
}
