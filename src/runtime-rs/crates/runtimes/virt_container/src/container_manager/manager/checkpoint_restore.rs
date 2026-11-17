// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::*;
use resource::rootfs::Rootfs;
use std::collections::{HashMap, HashSet};

use common::{
    types::{RestoredSandboxTask, SandboxCheckpointTask, SandboxRestoreTask},
    ContainerCheckpointRestore,
};

pub(super) struct PendingRestoredTask {
    checkpoint_key: String,
    rootfs: Arc<dyn Rootfs>,
    binding: bool,
}

impl VirtContainerManager {
    pub(super) async fn is_pending_restore(&self, container_id: &str) -> bool {
        self.pending_restored_tasks
            .read()
            .await
            .contains_key(container_id)
    }

    pub(super) async fn bind_restored_container(
        &self,
        config: ContainerConfig,
        spec: oci::Spec,
    ) -> Result<PID> {
        let container_id = config.container_id.clone();
        if self.containers.read().await.contains_key(&container_id) {
            return Err(anyhow!("container {container_id} already exists"));
        }

        let (restored_rootfs, checkpoint_key) = {
            let mut pending = self.pending_restored_tasks.write().await;
            let slot = pending.get_mut(&container_id).ok_or_else(|| {
                anyhow!("VM-restored task {container_id} has no pending restore record")
            })?;

            if slot.binding {
                return Err(anyhow!(
                    "VM-restored task {container_id} is already being bound"
                ));
            }

            slot.binding = true;
            (Arc::clone(&slot.rootfs), slot.checkpoint_key.clone())
        };

        let bind_result = async {
            let vmm_master_tid = self.get_vmm_master_tid().await?;
            let resources = spec
                .linux()
                .as_ref()
                .and_then(|linux| linux.resources().clone());

            let container = Container::new_restored(
                vmm_master_tid,
                config,
                spec,
                self.agent.clone(),
                self.resource_manager.clone(),
                restored_rootfs,
                self.use_guest_resolv_conf,
            )
            .await
            .context(format!(
                "construct VM-restored container from pending checkpoint {checkpoint_key}"
            ))?;

            // Publish the formal container and consume the pending slot while
            // both maps are locked. Readers can observe either Pending or
            // Bound, never an unowned gap between them.
            let mut containers = self.containers.write().await;
            let mut pending = self.pending_restored_tasks.write().await;
            if containers.contains_key(&container_id) {
                return Err(anyhow!("container {container_id} already exists"));
            }

            let slot = pending.get(&container_id).ok_or_else(|| {
                anyhow!("pending VM-restored task {container_id} disappeared during bind")
            })?;
            if !slot.binding {
                return Err(anyhow!(
                    "pending VM-restored task {container_id} left binding state"
                ));
            }

            // Finish fallible construction/validation before adopting resources.
            // After adoption, publication below has no await or fallible operation.
            self.resource_manager
                .adopt_restored_container(&container_id, resources.as_ref())
                .await
                .context("adopt restored task resource accounting")?;

            containers.insert(container_id.clone(), container);
            pending.remove(&container_id);

            Ok(PID {
                pid: vmm_master_tid,
            })
        }
        .await;

        if bind_result.is_err() {
            if let Some(slot) = self
                .pending_restored_tasks
                .write()
                .await
                .get_mut(&container_id)
            {
                slot.binding = false;
            }
        }

        bind_result
    }
}

#[async_trait]
impl ContainerCheckpointRestore for VirtContainerManager {
    async fn prepare_checkpoint_tasks(&self, tasks: &mut [SandboxCheckpointTask]) -> Result<()> {
        if tasks.is_empty() {
            return Err(anyhow!("checkpoint task inventory is empty"));
        }

        let containers = self.containers.read().await;
        if containers.len() != tasks.len() {
            return Err(anyhow!(
                "checkpoint inventory has {} tasks, sandbox has {}",
                tasks.len(),
                containers.len()
            ));
        }

        let (mut seen, mut seen_keys) = (HashSet::new(), HashSet::new());
        for task in tasks.iter_mut() {
            if task.checkpoint_key.is_empty()
                || !seen_keys.insert(task.checkpoint_key.as_str())
                || !seen.insert(task.task_id.as_str())
            {
                return Err(anyhow!("invalid or duplicate checkpoint task inventory"));
            }

            let container = containers
                .get(&task.task_id)
                .ok_or_else(|| anyhow!("checkpoint task {} is not in sandbox", task.task_id))?;

            let state = container
                .state_process(&ContainerProcess::new(&task.task_id, "")?)
                .await
                .context(format!("inspect checkpoint task {}", task.task_id))?;

            if state.status != ProcessStatus::Running {
                return Err(anyhow!(
                    "checkpoint task {} is {:?}, expected running",
                    task.task_id,
                    state.status
                ));
            }

            task.rootfs_device_ids = container
                .checkpoint_rootfs_ids()
                .await
                .context(format!("inspect checkpoint task {} rootfs", task.task_id))?;
        }

        Ok(())
    }

    async fn pause_checkpoint_tasks(&self, tasks: &[SandboxCheckpointTask]) -> Result<()> {
        let containers = self.containers.read().await;
        let mut paused = Vec::new();
        for task in tasks {
            let container = containers
                .get(&task.task_id)
                .ok_or_else(|| anyhow!("checkpoint task {} is not in sandbox", task.task_id))?;

            if let Err(err) = container.pause().await {
                let mut rollback_failures = Vec::new();
                for paused_id in paused.iter().rev() {
                    if let Some(paused_container) = containers.get(paused_id) {
                        if let Err(resume_err) = paused_container.resume().await {
                            rollback_failures.push(format!("{paused_id}: {resume_err:#}"));
                        }
                    }
                }

                let pause_error: Result<()> =
                    Err(err).context(format!("pause checkpoint task {}", task.task_id));
                return if rollback_failures.is_empty() {
                    pause_error
                } else {
                    pause_error.context(format!(
                        "failed to resume partially frozen tasks: {}",
                        rollback_failures.join(", ")
                    ))
                };
            }

            paused.push(task.task_id.clone());
        }

        Ok(())
    }

    async fn resume_checkpoint_tasks(&self, tasks: &[SandboxCheckpointTask]) -> Result<()> {
        let containers = self.containers.read().await;
        let mut first_error = None;

        for task in tasks {
            if let Some(container) = containers.get(&task.task_id) {
                if let Err(err) = container.resume().await {
                    first_error.get_or_insert(err);
                }
            }
        }

        first_error.map_or(Ok(()), Err)
    }

    async fn restore_tasks(
        &self,
        tasks: &[SandboxRestoreTask],
        restored: &[RestoredSandboxTask],
    ) -> Result<()> {
        if tasks.is_empty() || tasks.len() != restored.len() {
            return Err(anyhow!(
                "restore task inventory does not match restored sandbox"
            ));
        }

        let restored_by_key = restored
            .iter()
            .map(|task| (task.checkpoint_key.as_str(), task))
            .collect::<HashMap<_, _>>();
        if restored_by_key.len() != restored.len() {
            return Err(anyhow!("duplicate restored task checkpoint key"));
        }

        if !self.containers.read().await.is_empty() {
            return Err(anyhow!("restore requires an empty container manager"));
        }

        if !self.pending_restored_tasks.read().await.is_empty() {
            return Err(anyhow!("restore already has pending task records"));
        }

        let mut task_ids = HashSet::new();
        let mut rootfs_ids = HashSet::new();
        let mut pending_tasks = Vec::with_capacity(tasks.len());
        for task in tasks {
            let restored_task = restored_by_key
                .get(task.checkpoint_key.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "restore task {} is absent from restored sandbox",
                        task.checkpoint_key
                    )
                })?;

            if restored_task.task_id != task.task_id
                || restored_task.rootfs_device_ids.is_empty()
                || restored_task.rootfs_device_ids.iter().any(String::is_empty)
            {
                return Err(anyhow!(
                    "restored task {} has invalid identity or rootfs device",
                    task.task_id
                ));
            }

            if !task_ids.insert(task.task_id.as_str()) {
                return Err(anyhow!("duplicate restored task ID {}", task.task_id));
            }

            for device_id in &restored_task.rootfs_device_ids {
                if !rootfs_ids.insert(device_id.as_str()) {
                    return Err(anyhow!(
                        "restored rootfs device {} is assigned to multiple tasks",
                        device_id
                    ));
                }
            }

            let restored_rootfs = self
                .resource_manager
                .adopt_restored_rootfs(&restored_task.rootfs_device_ids)
                .await
                .context(format!("adopt restored rootfs for task {}", task.task_id))?;

            pending_tasks.push((
                task.task_id.clone(),
                PendingRestoredTask {
                    checkpoint_key: task.checkpoint_key.clone(),
                    rootfs: restored_rootfs,
                    binding: false,
                },
            ));
        }

        let containers = self.containers.read().await;
        let mut pending = self.pending_restored_tasks.write().await;
        if !containers.is_empty() {
            return Err(anyhow!("restore requires an empty container manager"));
        }

        if !pending.is_empty() {
            return Err(anyhow!("restore already has pending task records"));
        }

        for (task_id, task) in pending_tasks {
            pending.insert(task_id, task);
        }

        Ok(())
    }
}
