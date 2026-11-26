// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

//! Shared blocking workers for private restore artifact preparation.

use anyhow::{Context, Result};
use tokio::sync::Semaphore;

pub(in crate::sandbox::checkpoint_restore) const RESTORE_ARTIFACT_CONCURRENCY: usize = 2;
pub(in crate::sandbox::checkpoint_restore) static RESTORE_ARTIFACT_SEMAPHORE: Semaphore =
    Semaphore::const_new(RESTORE_ARTIFACT_CONCURRENCY);

pub(in crate::sandbox::checkpoint_restore) async fn run_artifact_worker<F>(worker: F) -> Result<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    let permit = RESTORE_ARTIFACT_SEMAPHORE
        .acquire()
        .await
        .context("acquire private restore artifact worker permit")?;

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        worker()
    })
    .await
    .context("private restore artifact worker failed")?
}
