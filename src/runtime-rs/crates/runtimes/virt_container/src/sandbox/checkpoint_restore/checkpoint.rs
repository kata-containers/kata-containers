// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, fs, path::Path, time::Instant};

use anyhow::{anyhow, Context, Result};
use common::{
    error::Error as CommonError,
    types::{CheckpointSandboxRequest, SandboxCheckpointTask},
};
use hypervisor::{VmCheckpointTimeout, VmSaveOptions, VmSaveResult};

use crate::sandbox::VirtSandbox;

use super::{
    artifacts::{rootfs, vm},
    checkpoint_files,
    manifest::{
        CheckpointManifest, CheckpointRootfs, CheckpointTask, WORKLOAD_CHECKPOINT_FORMAT_VERSION,
        WORKLOAD_CHECKPOINT_STATE, WORKLOAD_CHECKPOINT_STORAGE,
    },
    publish_manifest,
};

// Keep the saved memory handles alive until VM recovery has been attempted,
// even when capturing the rootfs fails.
pub(super) struct PausedVmSave<T = VmSaveResult> {
    vm: T,
    rootfs: Result<Vec<CheckpointRootfs>>,
}

impl<T> PausedVmSave<T> {
    pub(super) fn into_result(self) -> Result<(Vec<CheckpointRootfs>, T)> {
        let Self { vm, rootfs } = self;
        rootfs.map(|rootfs| (rootfs, vm))
    }
}

impl VirtSandbox {
    pub(super) async fn checkpoint_workload(
        &self,
        req: &CheckpointSandboxRequest,
        owns_output: &mut bool,
    ) -> Result<()> {
        let started = Instant::now();

        // Re-read the resource inventory after tasks are frozen. Static request
        // validation was already completed before the transaction paused them.
        let network_macs = self.validate_checkpoint_devices(req).await?;

        let (state_path, manifest_path) = checkpoint_files(&req.output_path)?;
        fs::create_dir(&state_path).context("create VM checkpoint directory")?;
        *owns_output = true;

        let hypervisor_name = self
            .resource_manager
            .config()
            .await
            .runtime
            .hypervisor_name
            .clone();

        let agent_address = self
            .hypervisor
            .get_agent_socket()
            .await
            .context("get agent socket before checkpoint")?;

        let probe_suppression = self.monitor.suppress_for_disconnect();
        if let Err(disconnect_err) = self.agent.disconnect().await {
            // Disconnect can clear the RPC client before returning an error.
            // Restore the connection before the caller resumes the source tasks.
            let reconnect = self.agent.start(&agent_address).await;
            return complete_checkpoint(
                Err(disconnect_err).context("disconnect agent before checkpoint"),
                reconnect.context("restart Agent RPC and log forwarding after failed disconnect"),
            );
        }

        if let Err(pause_err) = self.hypervisor.pause_vm().await {
            return match self.agent.start(&agent_address).await {
                Ok(()) => Err(pause_err).context("pause VM"),
                Err(reconnect_err) => Err(pause_err).context(format!(
                    "pause VM; reconnect agent after pause failure also failed: {reconnect_err:#}"
                )),
            };
        }

        let checkpoint_result = self.save_paused_vm(req, &state_path).await;
        // Always attempt recovery, including when saving the VM or rootfs failed.
        let recovery_result = self.resume_checkpoint_vm(&agent_address).await;

        drop(probe_suppression);

        let checkpoint_result = checkpoint_result.and_then(PausedVmSave::into_result);
        let (rootfs, mut vm_save_result) = complete_checkpoint(checkpoint_result, recovery_result)?;

        let resource_topology = vm_save_result
            .resource_topology
            .take()
            .ok_or_else(|| anyhow!("workload save returned no VM resource topology"))?;

        if resource_topology.vcpus == 0 || resource_topology.memory_mb == 0 {
            return Err(anyhow!(
                "workload save returned invalid VM resource topology: {:?}",
                resource_topology
            ));
        }

        let mut vm_state =
            vm::finalize_checkpoint_artifacts(&hypervisor_name, vm_save_result, state_path.clone())
                .await?;

        for artifact in &mut vm_state {
            artifact.file = format!("{WORKLOAD_CHECKPOINT_STATE}/{}", artifact.file);
        }

        let runtime_fingerprint = self.runtime_fingerprint(&req.options).await?;

        let manifest = CheckpointManifest {
            format_version: WORKLOAD_CHECKPOINT_FORMAT_VERSION,
            hypervisor: hypervisor_name,
            tasks: req
                .tasks
                .iter()
                .map(|task| CheckpointTask {
                    checkpoint_key: task.checkpoint_key.clone(),
                    task_id: task.task_id.clone(),
                })
                .collect(),
            rootfs,
            network_macs,
            vm_vcpus: resource_topology.vcpus,
            vm_memory_mb: resource_topology.memory_mb,
            vm_state,
            runtime_fingerprint,
            template: self.template_identity.clone(),
        };

        publish_manifest(&manifest, &manifest_path)?;
        info!(
            sl!(),
            "checkpoint completed";
            "elapsed_us" => started.elapsed().as_micros(),
        );

        Ok(())
    }

    pub(super) async fn cleanup_checkpoint(&self, req: &CheckpointSandboxRequest) -> Result<()> {
        let (state_path, manifest_path) = checkpoint_files(&req.output_path)?;

        let paths = [
            manifest_path.clone(),
            manifest_path.with_extension("json.partial"),
        ];

        let mut failures = Vec::new();
        for path in paths {
            if let Err(err) = fs::remove_file(&path) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    failures.push(format!("{}: {err}", path.display()));
                }
            }
        }

        let storage_dir = Path::new(&req.output_path).join(WORKLOAD_CHECKPOINT_STORAGE);
        if let Err(err) = fs::remove_dir_all(&storage_dir) {
            if err.kind() != std::io::ErrorKind::NotFound {
                failures.push(format!("{}: {err}", storage_dir.display()));
            }
        }

        // Keep the output reserved until cleanup has finished touching its siblings.
        if let Err(err) = fs::remove_dir_all(&state_path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                failures.push(format!("{}: {err}", state_path.display()));
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "remove incomplete checkpoint files: {}",
                failures.join(", ")
            ))
        }
    }

    pub(in crate::sandbox::checkpoint_restore) async fn checkpoint_rootfs_artifacts(
        &self,
        checkpoint_root: &Path,
        tasks: &[SandboxCheckpointTask],
        options: &HashMap<String, String>,
    ) -> Result<Vec<CheckpointRootfs>> {
        let device_manager = self.resource_manager.get_device_manager().await;
        let blocks = device_manager.read().await.checkpoint_block_devices().await;

        let checkpoint_root = checkpoint_root.to_path_buf();
        let tasks = tasks.to_vec();
        let options = options.clone();

        tokio::task::spawn_blocking(move || {
            rootfs::write_rootfs_artifacts(&checkpoint_root, &tasks, &options, blocks)
        })
        .await
        .context("checkpoint rootfs artifact worker failed")?
    }

    pub(super) async fn save_paused_vm(
        &self,
        req: &CheckpointSandboxRequest,
        state_path: &Path,
    ) -> Result<PausedVmSave> {
        let saved = self
            .hypervisor
            .save_vm(VmSaveOptions::Workload {
                state_path: state_path.to_path_buf(),
                timeout: req.operation_timeout,
            })
            .await
            .map_err(|err| -> anyhow::Error {
                if let Some(timeout) = err.downcast_ref::<VmCheckpointTimeout>() {
                    CommonError::SandboxOperationDeadline(timeout.to_string()).into()
                } else {
                    err.context("save workload checkpoint")
                }
            })?;

        let rootfs = self
            .checkpoint_rootfs_artifacts(Path::new(&req.output_path), &req.tasks, &req.options)
            .await
            .context("save workload checkpoint rootfs");

        Ok(PausedVmSave { rootfs, vm: saved })
    }

    pub(super) async fn resume_checkpoint_vm(&self, address: &str) -> Result<()> {
        self.hypervisor
            .resume_vm()
            .await
            .context("resume VM after checkpoint")?;

        self.agent
            .start(address)
            .await
            .context("reconnect agent after checkpoint")
    }
}

// Keep the original save error when recovery also fails.
pub(super) fn complete_checkpoint<T>(saved: Result<T>, recovered: Result<()>) -> Result<T> {
    match (saved, recovered) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(err), Ok(())) => Err(err),
        (Ok(_), Err(err)) => Err(err),
        (Err(err), Err(recovery_err)) => {
            Err(err).context(format!("checkpoint recovery also failed: {recovery_err:#}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::sync::Arc;

    #[test]
    fn rootfs_failure_keeps_saved_resources_until_result_is_consumed() {
        let saved_resource = Arc::new(());
        let retained = Arc::downgrade(&saved_resource);
        let saved = PausedVmSave {
            vm: saved_resource,
            rootfs: Err(anyhow!("rootfs capture failed")),
        };

        // Recovery is attempted while PausedVmSave still owns the saved resources.
        assert!(retained.upgrade().is_some());
        let err = saved.into_result().err().expect("rootfs capture must fail");
        assert_eq!(err.to_string(), "rootfs capture failed");
        assert!(retained.upgrade().is_none());
    }

    #[test]
    fn recovery_error_preserves_the_original_checkpoint_error() {
        let err =
            complete_checkpoint::<()>(Err(anyhow!("save failed")), Err(anyhow!("resume failed")))
                .unwrap_err();
        assert_eq!(err.root_cause().to_string(), "save failed");
        assert!(format!("{err:#}").contains("resume failed"));
        assert!(complete_checkpoint(Ok(()), Err(anyhow!("reconnect failed"))).is_err());
        assert!(complete_checkpoint::<()>(Err(anyhow!("save failed")), Ok(())).is_err());
        assert_eq!(complete_checkpoint(Ok(42), Ok(())).unwrap(), 42);
    }
}
