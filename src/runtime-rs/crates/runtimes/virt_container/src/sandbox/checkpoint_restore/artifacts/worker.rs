// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    use tokio::{sync::oneshot, time::timeout};
    #[tokio::test]
    async fn blocking_worker_keeps_permit_after_async_cancellation() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let task = tokio::spawn(run_artifact_worker(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        }));
        started_rx.await.unwrap();
        task.abort();

        assert!(timeout(
            Duration::from_millis(100),
            RESTORE_ARTIFACT_SEMAPHORE.acquire_many(RESTORE_ARTIFACT_CONCURRENCY as u32),
        )
        .await
        .is_err());

        release_tx.send(()).unwrap();
        let permits = timeout(
            Duration::from_secs(1),
            RESTORE_ARTIFACT_SEMAPHORE.acquire_many(RESTORE_ARTIFACT_CONCURRENCY as u32),
        )
        .await
        .unwrap()
        .unwrap();
        drop(permits);
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
