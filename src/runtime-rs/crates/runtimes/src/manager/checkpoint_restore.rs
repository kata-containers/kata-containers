// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use common::{
    error::Error as CommonError,
    types::{CheckpointSandboxRequest, RestoreSandboxRequest, SandboxResponse},
    CheckpointRestoreRuntime,
};
use tokio::sync::{RwLock, RwLockWriteGuard};

use super::RuntimeHandlerManager;

pub(super) async fn acquire_operation_guard(
    gate: &RwLock<()>,
    timeout: Option<Duration>,
) -> Result<(RwLockWriteGuard<'_, ()>, Option<Duration>)> {
    let started = Instant::now();
    let guard = match timeout {
        Some(timeout) => tokio::time::timeout(timeout, gate.write())
            .await
            .map_err(|_| {
                CommonError::SandboxOperationDeadline(format!(
                    "timed out waiting {:?} for the sandbox operation gate",
                    timeout
                ))
            })?,
        None => gate.write().await,
    };

    let remaining = timeout.map(|timeout| timeout.saturating_sub(started.elapsed()));

    Ok((guard, remaining))
}

impl RuntimeHandlerManager {
    pub(super) async fn handle_restore_sandbox(
        &self,
        req: Box<RestoreSandboxRequest>,
    ) -> Result<SandboxResponse> {
        let runtime_id = self.inner.read().await.id.clone();
        if req.sandbox_config.sandbox_id != runtime_id {
            return Err(CommonError::InvalidSandboxOperation(format!(
                "restore sandbox id {} does not match shim id {}",
                req.sandbox_config.sandbox_id, runtime_id
            ))
            .into());
        }

        let instance = self
            .get_runtime_instance()
            .await
            .context("get runtime instance for sandbox restore")?;

        if instance.sandbox.status().await?.created_at.is_none() {
            return Err(CommonError::SandboxOperationPrecondition(
                "restore requires a sandbox prepared and started from its checkpoint".to_string(),
            )
            .into());
        }

        let restore_future = async {
            let checkpoint_restore = instance.checkpoint_restore.as_ref().ok_or_else(|| {
                CommonError::SandboxOperationUnsupported(
                    "sandbox restore is unsupported by this runtime".to_string(),
                )
            })?;

            let info = checkpoint_restore
                .sandbox
                .restore_sandbox(&req)
                .await
                .context("restore sandbox")?;
            checkpoint_restore
                .container_manager
                .restore_tasks(&req.tasks, &info.tasks)
                .await
                .context("restore sandbox task records")?;

            Ok::<_, anyhow::Error>(info)
        };

        let restore_result = if let Some(timeout) = req.operation_timeout {
            match tokio::time::timeout(timeout, restore_future).await {
                Ok(result) => result,
                Err(_) => Err(CommonError::SandboxOperationDeadline(format!(
                    "sandbox restore exceeded its {:?} runtime budget",
                    timeout
                ))
                .into()),
            }
        } else {
            restore_future.await
        };

        match restore_result {
            Ok(info) => Ok(SandboxResponse::RestoreSandbox(info)),
            Err(restore_err) => {
                let stop_result = instance.sandbox.stop().await;
                let cleanup_result = instance.sandbox.cleanup().await;
                let shutdown_result = instance.sandbox.shutdown().await;

                let rollback_error = stop_result
                    .err()
                    .map(|err| format!("stop restored sandbox: {err:#}"))
                    .or_else(|| {
                        cleanup_result
                            .err()
                            .map(|err| format!("clean up restored sandbox: {err:#}"))
                    })
                    .or_else(|| {
                        shutdown_result
                            .err()
                            .map(|err| format!("shut down restored sandbox: {err:#}"))
                    });

                match rollback_error {
                    Some(rollback_error) => Err(restore_err).context(format!(
                        "failed to discard restored sandbox: {rollback_error}"
                    )),
                    None => Err(restore_err),
                }
            }
        }
    }

    pub(super) async fn handle_checkpoint_sandbox(
        &self,
        req: CheckpointSandboxRequest,
    ) -> Result<SandboxResponse> {
        let instance = self
            .get_runtime_instance()
            .await
            .context("get runtime instance")?;

        let checkpoint_restore = instance.checkpoint_restore.as_ref().ok_or_else(|| {
            CommonError::SandboxOperationUnsupported(
                "sandbox checkpoint is unsupported by this runtime".to_string(),
            )
        })?;

        run_checkpoint_transaction(checkpoint_restore, req).await
    }
}

async fn run_checkpoint_transaction(
    checkpoint_restore: &CheckpointRestoreRuntime,
    mut req: CheckpointSandboxRequest,
) -> Result<SandboxResponse> {
    let started = Instant::now();
    let operation_timeout = req.operation_timeout;
    let sandbox_ckpt_restore = checkpoint_restore.sandbox.clone();
    let cm = checkpoint_restore.container_manager.clone();

    cm.prepare_checkpoint_tasks(&mut req.tasks)
        .await
        .map_err(|err| {
            CommonError::SandboxOperationPrecondition(format!(
                "cannot prepare checkpoint inventory: {err:#}"
            ))
        })?;

    sandbox_ckpt_restore
        .validate_checkpoint_sandbox(&req)
        .await
        .context("validate checkpoint sandbox before freezing tasks")?;

    cm.pause_checkpoint_tasks(&req.tasks).await.map_err(|err| {
        CommonError::SandboxOperationPrecondition(format!(
            "cannot freeze checkpoint inventory: {err:#}"
        ))
    })?;

    if let Some(timeout) = operation_timeout {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            let timeout_err: anyhow::Error = CommonError::SandboxOperationDeadline(format!(
                "sandbox checkpoint exceeded its {:?} runtime budget",
                timeout
            ))
            .into();

            return match cm.resume_checkpoint_tasks(&req.tasks).await {
                Ok(()) => Err(timeout_err),
                Err(resume_err) => Err(timeout_err).context(format!(
                    "resume checkpoint tasks after deadline; resume also failed: {resume_err:#}"
                )),
            };
        }
        req.operation_timeout = Some(remaining);
    }

    let mut owns_output = false;
    let checkpoint_result = sandbox_ckpt_restore
        .checkpoint_sandbox(&req, &mut owns_output)
        .await;
    let resume_result = cm.resume_checkpoint_tasks(&req.tasks).await;

    let cleanup_result = if owns_output && (checkpoint_result.is_err() || resume_result.is_err()) {
        sandbox_ckpt_restore.cleanup_sandbox_checkpoint(&req).await
    } else {
        Ok(())
    };

    if let Err(checkpoint_err) = checkpoint_result {
        let cleanup_context = match (resume_result, cleanup_result) {
            (Ok(()), Ok(())) => String::new(),
            (Err(resume_err), Ok(())) => {
                format!("; source task resume also failed: {resume_err:#}")
            }
            (Ok(()), Err(cleanup_err)) => {
                format!("; checkpoint cleanup also failed: {cleanup_err:#}")
            }
            (Err(resume_err), Err(cleanup_err)) => format!(
                "; source task resume also failed: {resume_err:#}; checkpoint cleanup also failed: {cleanup_err:#}"
            ),
        };

        return Err(checkpoint_err).context(format!("checkpoint sandbox{cleanup_context}"));
    }

    if let Err(resume_err) = resume_result {
        return match cleanup_result {
            Ok(()) => Err(resume_err).context("resume checkpoint tasks"),
            Err(cleanup_err) => Err(resume_err).context(format!(
                "resume checkpoint tasks; checkpoint cleanup also failed: {cleanup_err:#}"
            )),
        };
    }

    Ok(SandboxResponse::CheckpointSandbox)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use common::{
        error::Error as CommonError,
        types::{
            CheckpointSandboxRequest, PrepareRestoreOptions, RestoreSandboxInfo,
            RestoreSandboxRequest, RestoredSandboxTask, SandboxCheckpointTask, SandboxRestoreTask,
        },
        CheckpointRestoreRuntime, ContainerCheckpointRestore, SandboxCheckpointRestore,
    };
    use tokio::sync::RwLock;

    use super::{acquire_operation_guard, run_checkpoint_transaction};

    struct TestCheckpointSandbox {
        calls: Arc<Mutex<Vec<&'static str>>>,
        owns_output: bool,
    }

    #[async_trait]
    impl SandboxCheckpointRestore for TestCheckpointSandbox {
        async fn prepare_restore_sandbox(&self, _: &PrepareRestoreOptions) -> Result<()> {
            unreachable!()
        }

        async fn validate_checkpoint_sandbox(&self, _: &CheckpointSandboxRequest) -> Result<()> {
            self.calls.lock().unwrap().push("validate");
            Ok(())
        }

        async fn checkpoint_sandbox(
            &self,
            _: &CheckpointSandboxRequest,
            owns_output: &mut bool,
        ) -> Result<()> {
            self.calls.lock().unwrap().push("checkpoint");
            *owns_output = self.owns_output;
            Err(anyhow!("forced checkpoint failure"))
        }

        async fn restore_sandbox(&self, _: &RestoreSandboxRequest) -> Result<RestoreSandboxInfo> {
            unreachable!()
        }

        async fn cleanup_sandbox_checkpoint(&self, _: &CheckpointSandboxRequest) -> Result<()> {
            self.calls.lock().unwrap().push("cleanup");
            Ok(())
        }
    }

    struct TestCheckpointContainers {
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl ContainerCheckpointRestore for TestCheckpointContainers {
        async fn prepare_checkpoint_tasks(&self, _: &mut [SandboxCheckpointTask]) -> Result<()> {
            self.calls.lock().unwrap().push("prepare");
            Ok(())
        }

        async fn pause_checkpoint_tasks(&self, _: &[SandboxCheckpointTask]) -> Result<()> {
            self.calls.lock().unwrap().push("pause");
            Ok(())
        }

        async fn resume_checkpoint_tasks(&self, _: &[SandboxCheckpointTask]) -> Result<()> {
            self.calls.lock().unwrap().push("resume");
            Ok(())
        }

        async fn restore_tasks(
            &self,
            _: &[SandboxRestoreTask],
            _: &[RestoredSandboxTask],
        ) -> Result<()> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn operation_gate_wait_is_bounded() {
        let gate = RwLock::new(());
        let _reader = gate.read().await;

        let result = acquire_operation_guard(&gate, Some(Duration::from_millis(10))).await;
        let error = match result {
            Ok(_) => panic!("operation gate unexpectedly acquired"),
            Err(error) => error,
        };

        assert!(matches!(
            error.downcast_ref::<CommonError>(),
            Some(CommonError::SandboxOperationDeadline(_))
        ));
    }

    #[tokio::test]
    async fn operation_gate_wait_consumes_the_request_budget() {
        let gate = Arc::new(RwLock::new(()));
        let reader = gate.clone().read_owned().await;
        let release = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            drop(reader);
        });
        let budget = Duration::from_secs(1);

        let (_guard, remaining) = acquire_operation_guard(&gate, Some(budget))
            .await
            .expect("operation gate should be acquired");
        release.await.expect("reader task should finish");

        assert!(remaining.expect("bounded request should retain a budget") < budget);
    }

    #[tokio::test]
    async fn checkpoint_failure_resumes_tasks_before_cleanup() {
        assert_eq!(
            checkpoint_failure_calls(true).await,
            [
                "prepare",
                "validate",
                "pause",
                "checkpoint",
                "resume",
                "cleanup"
            ]
        );
    }

    #[tokio::test]
    async fn checkpoint_failure_keeps_output_owned_by_another_attempt() {
        assert_eq!(
            checkpoint_failure_calls(false).await,
            ["prepare", "validate", "pause", "checkpoint", "resume"]
        );
    }

    async fn checkpoint_failure_calls(owns_output: bool) -> Vec<&'static str> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = CheckpointRestoreRuntime {
            sandbox: Arc::new(TestCheckpointSandbox {
                calls: calls.clone(),
                owns_output,
            }),
            container_manager: Arc::new(TestCheckpointContainers {
                calls: calls.clone(),
            }),
        };
        let request = CheckpointSandboxRequest {
            sandbox_id: "sandbox".to_string(),
            output_path: "/checkpoint".to_string(),
            tasks: Vec::new(),
            options: HashMap::new(),
            operation_timeout: None,
        };

        let result = run_checkpoint_transaction(&runtime, request).await;

        assert!(result.is_err());
        let observed = calls.lock().unwrap();
        observed.clone()
    }
}
