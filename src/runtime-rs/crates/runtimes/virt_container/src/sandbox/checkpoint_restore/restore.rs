// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use common::{
    error::Error as CommonError,
    types::{
        PrepareRestoreOptions, RestoreSandboxInfo, RestoreSandboxRequest, RestoredSandboxTask,
    },
};

use crate::sandbox::VirtSandbox;

use super::{
    artifacts::{self, rootfs, vm},
    manifest::CheckpointManifest,
    PendingRestore, RestoreBoot,
};

use crate::sandbox::startup::StartMode;
use hypervisor::HYPERVISOR_DRAGONBALL;
use resource::ResourceConfig;
use std::time::Duration;
use tokio::time::timeout;

impl VirtSandbox {
    pub(super) async fn prepare_restore_request(
        &self,
        options: &PrepareRestoreOptions,
    ) -> Result<()> {
        if options.checkpoint_path.is_empty() {
            return Err(CommonError::InvalidSandboxOperation(
                "restore checkpoint path is empty".to_string(),
            )
            .into());
        }

        let mut state = self.restore_state.write().await;
        if state.is_restoring() {
            return Err(CommonError::InvalidSandboxOperation(
                "sandbox restore is already prepared".to_string(),
            )
            .into());
        }

        state.set_prepare(options.clone());

        Ok(())
    }

    pub(super) async fn restore_workload(
        &self,
        req: &RestoreSandboxRequest,
    ) -> Result<RestoreSandboxInfo> {
        let started = Instant::now();
        let info = self.rebind_restore(req).await?;
        info!(
            sl!(),
            "restore rebind completed";
            "elapsed_us" => started.elapsed().as_micros(),
        );

        Ok(info)
    }

    pub(in crate::sandbox) async fn prepare_restore_start(&self) -> Result<()> {
        let prepare = self
            .restore_state
            .write()
            .await
            .take_prepare()
            .ok_or_else(|| {
                CommonError::InvalidSandboxOperation(
                    "sandbox restore preparation is unavailable".to_string(),
                )
            })?;

        let manifest = self.load_restore_manifest(&prepare.checkpoint_path).await?;
        let sandbox_config = self
            .sandbox_config
            .as_ref()
            .ok_or_else(|| anyhow!("sandbox config is missing"))?;

        let checkpoint_root = Path::new(&prepare.checkpoint_path);

        debug!(sl!(), "create restore rootfs overlay");

        let restore_dir =
            artifacts::restore_artifact_dir(checkpoint_root, &sandbox_config.sandbox_id)?;
        let is_dragonball = manifest.hypervisor == hypervisor::HYPERVISOR_DRAGONBALL;

        self.create_private_restore_dir(&restore_dir).await?;

        // Runtime compatibility and private artifact preparation are independent.
        // Wait for every blocking worker to settle before returning an error so
        // rollback cannot race an in-flight copy or verification.
        let artifacts = async {
            let (blocks, state_dir) = tokio::join!(
                rootfs::restore_rootfs_artifacts(
                    checkpoint_root,
                    &manifest.tasks,
                    &prepare.options,
                    &manifest.rootfs,
                    &restore_dir,
                    !is_dragonball
                ),
                vm::restore_vm_artifacts(
                    &manifest.hypervisor,
                    checkpoint_root,
                    &manifest.vm_state,
                    &restore_dir,
                    hypervisor::VmResourceTopology {
                        vcpus: manifest.vm_vcpus,
                        memory_mb: manifest.vm_memory_mb,
                    },
                ),
            );

            let blocks = blocks.context("restore managed rootfs artifacts")?;
            let state_dir = state_dir.context("restore VM state artifacts")?;

            Ok::<_, anyhow::Error>((blocks, state_dir))
        };

        let (runtime_check, artifacts) = tokio::join!(
            self.verify_runtime_fingerprint(&manifest, &prepare.options),
            artifacts,
        );

        runtime_check?;

        let (mut blocks, state_dir) = artifacts?;
        blocks.sort_by_key(|block| block.config.index);

        let options = hypervisor::VmRestoreOptions {
            state_path: state_dir,
            network_macs: manifest.network_macs.clone(),
            resource_topology: hypervisor::VmResourceTopology {
                vcpus: manifest.vm_vcpus,
                memory_mb: manifest.vm_memory_mb,
            },
        };

        {
            let mut state = self.restore_state.write().await;
            state.set_pending(PendingRestore {
                boot: Some(RestoreBoot { options, blocks }),
                checkpoint_path: prepare.checkpoint_path,
                options: prepare.options,
                manifest,
            });
        }

        Ok(())
    }

    async fn rebind_restore(&self, req: &RestoreSandboxRequest) -> Result<RestoreSandboxInfo> {
        let pending = self
            .restore_state
            .write()
            .await
            .take_pending()
            .ok_or_else(|| {
                CommonError::InvalidSandboxOperation(
                    "sandbox restore state is unavailable".to_string(),
                )
            })?;

        if pending.checkpoint_path != req.checkpoint_path || pending.options != req.options {
            return Err(CommonError::InvalidSandboxOperation(
                "restore request does not match prepared checkpoint".to_string(),
            )
            .into());
        }

        if pending.boot.is_some() {
            return Err(CommonError::SandboxOperationPrecondition(
                "restored VM has not consumed its prepared boot state".to_string(),
            )
            .into());
        }

        let (mappings, tasks) = map_restore_tasks(req, &pending.manifest)?;

        self.agent
            .rebind_sandbox(agent::RebindSandboxRequest {
                sandbox_id: self.sid.clone(),
                containers: mappings,
                hostname: req.sandbox_config.hostname.clone(),
                dns: req.sandbox_config.dns.clone(),
            })
            .await
            .map_err(|err| {
                if agent::is_failed_precondition(&err) {
                    CommonError::SandboxOperationPrecondition(format!(
                        "restored guest rejected sandbox/task rebinding: {err:#}"
                    ))
                    .into()
                } else {
                    err.context("rebind restored guest sandbox and task ids")
                }
            })?;

        self.start_restore_watchers().await;

        Ok(RestoreSandboxInfo { tasks })
    }
}

fn map_restore_tasks(
    req: &RestoreSandboxRequest,
    manifest: &CheckpointManifest,
) -> Result<(Vec<agent::ContainerIDMapping>, Vec<RestoredSandboxTask>)> {
    if manifest.tasks.len() != req.tasks.len() || req.tasks.is_empty() {
        return Err(CommonError::InvalidSandboxOperation(
            "restore task inventory does not match checkpoint".to_string(),
        )
        .into());
    }

    let source_tasks = manifest
        .tasks
        .iter()
        .map(|task| (task.checkpoint_key.as_str(), task.task_id.as_str()))
        .collect::<HashMap<_, _>>();

    if source_tasks.len() != manifest.tasks.len() {
        return Err(CommonError::InvalidSandboxOperation(
            "checkpoint contains duplicate task keys".to_string(),
        )
        .into());
    }

    let source_rootfs = manifest
        .rootfs
        .iter()
        .map(|rootfs| (rootfs.checkpoint_key.as_str(), rootfs))
        .collect::<HashMap<_, _>>();

    if source_rootfs.len() != manifest.rootfs.len()
        || source_rootfs.len() != source_tasks.len()
        || source_rootfs.values().any(|rootfs| {
            rootfs.device_id.is_empty()
                || rootfs
                    .immutable_lower
                    .as_ref()
                    .is_some_and(|lower| lower.device_id.is_empty())
        })
    {
        return Err(CommonError::InvalidSandboxOperation(
            "checkpoint contains invalid rootfs inventory".to_string(),
        )
        .into());
    }

    let task_count = req.tasks.len();
    let mut mappings = Vec::with_capacity(task_count);
    let mut tasks = Vec::with_capacity(task_count);
    let mut restore_keys = HashSet::with_capacity(task_count);
    let mut new_ids = HashSet::with_capacity(task_count);

    for task in &req.tasks {
        if task.checkpoint_key.is_empty() || !restore_keys.insert(task.checkpoint_key.as_str()) {
            return Err(CommonError::InvalidSandboxOperation(
                "invalid or duplicate restore checkpoint key".to_string(),
            )
            .into());
        }

        let old_id = source_tasks
            .get(task.checkpoint_key.as_str())
            .ok_or_else(|| {
                CommonError::InvalidSandboxOperation(format!(
                    "restore task key {} is absent",
                    task.checkpoint_key
                ))
            })?;

        let rootfs = source_rootfs
            .get(task.checkpoint_key.as_str())
            .ok_or_else(|| {
                CommonError::InvalidSandboxOperation(format!(
                    "restore task key {} has no rootfs",
                    task.checkpoint_key
                ))
            })?;

        if task.task_id.is_empty() || !new_ids.insert(task.task_id.as_str()) {
            return Err(CommonError::InvalidSandboxOperation(format!(
                "invalid or duplicate restored task id {}",
                task.task_id
            ))
            .into());
        }

        let mut rootfs_device_ids =
            Vec::with_capacity(1 + usize::from(rootfs.immutable_lower.is_some()));
        rootfs_device_ids.push(rootfs.device_id.clone());
        if let Some(lower) = &rootfs.immutable_lower {
            rootfs_device_ids.push(lower.device_id.clone());
        }

        // Process and resource config is already in the checkpointed guest.
        mappings.push(agent::ContainerIDMapping {
            old_id: (*old_id).to_string(),
            new_id: task.task_id.clone(),
        });

        tasks.push(RestoredSandboxTask {
            checkpoint_key: task.checkpoint_key.clone(),
            task_id: task.task_id.clone(),
            rootfs_device_ids,
        });
    }

    Ok((mappings, tasks))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn restore_inventory() -> (RestoreSandboxRequest, CheckpointManifest) {
        let rootfs = |key: &str| {
            serde_json::json!({
                "checkpoint_key": key, "file": format!("storage/{key}.sparse"),
                "format": "kata-sparse-raw-v1", "device_id": format!("dev-{key}"),
                "index": 1, "driver_option": "virtio-blk", "virt_path": "/dev/vdb",
                "is_direct": null, "discard_unmap": false, "aio": "threads",
                "queue_size": 128, "num_queues": 1, "logical_sector_size": 512,
                "physical_sector_size": 512, "serial_override": "",
                "logical_size": 4096, "encoded_size": 32, "sha256": "a".repeat(64)
            })
        };
        let mut a = rootfs("a");
        a["immutable_lower"] = serde_json::json!({
            "device_id": "lower-a", "index": 2, "driver_option": "virtio-blk",
            "virt_path": "/dev/vdc", "is_direct": null, "discard_unmap": false,
            "aio": "threads", "queue_size": 128, "num_queues": 1,
            "logical_sector_size": 512, "physical_sector_size": 512,
            "serial_override": "", "size": 4096, "sha256": "b".repeat(64)
        });
        let manifest = serde_json::from_value(serde_json::json!({
            "format_version": 16, "hypervisor": "qemu",
            "tasks": [
                {"checkpoint_key": "a", "task_id": "old-a"},
                {"checkpoint_key": "b", "task_id": "old-b"}
            ],
            "rootfs": [a, rootfs("b")], "network_macs": [],
            "vm_vcpus": 1, "vm_memory_mb": 128, "vm_state": [],
            "runtime_fingerprint": "test"
        }))
        .unwrap();
        let request = RestoreSandboxRequest {
            sandbox_config: common::types::SandboxConfig {
                sandbox_id: "inventory-test".into(),
                hostname: "inventory-test".into(),
                dns: Vec::new(),
                network_env: common::SandboxNetworkEnv::default(),
                annotations: HashMap::new(),
                hooks: None,
                state: runtime_spec::State {
                    version: Default::default(),
                    id: String::new(),
                    status: runtime_spec::ContainerState::Creating,
                    pid: 0,
                    bundle: String::new(),
                    annotations: Default::default(),
                },
                shm_size: 0,
                config_path: None,
            },
            checkpoint_path: "/unused-inventory-test".into(),
            options: HashMap::new(),
            tasks: ["b", "a"]
                .iter()
                .map(|&key| common::types::SandboxRestoreTask {
                    checkpoint_key: key.into(),
                    task_id: format!("new-{key}"),
                })
                .collect(),
            operation_timeout: None,
        };
        (request, manifest)
    }

    #[test]
    fn restore_mapping_preserves_task_order() {
        let (request, manifest) = restore_inventory();
        let (mappings, tasks) = map_restore_tasks(&request, &manifest).unwrap();
        assert_eq!(
            mappings
                .iter()
                .map(|mapping| (mapping.old_id.as_str(), mapping.new_id.as_str()))
                .collect::<Vec<_>>(),
            [("old-b", "new-b"), ("old-a", "new-a")]
        );
        assert_eq!(
            tasks
                .iter()
                .map(|task| (task.checkpoint_key.as_str(), task.task_id.as_str()))
                .collect::<Vec<_>>(),
            [("b", "new-b"), ("a", "new-a")]
        );
        assert_eq!(tasks[0].rootfs_device_ids, ["dev-b"]);
        assert_eq!(tasks[1].rootfs_device_ids, ["dev-a", "lower-a"]);
    }

    #[test]
    fn restore_mapping_preserves_error_order() {
        for case in 0..16 {
            let (mut request, mut manifest) = restore_inventory();
            let expected = match case {
                0 => {
                    request.tasks.pop();
                    "restore task inventory does not match checkpoint"
                }
                1 => {
                    request.tasks.clear();
                    manifest.tasks.clear();
                    manifest.rootfs.clear();
                    "restore task inventory does not match checkpoint"
                }
                2 => {
                    manifest.tasks[1].checkpoint_key = "a".into();
                    "checkpoint contains duplicate task keys"
                }
                3 => {
                    manifest.rootfs.pop();
                    "checkpoint contains invalid rootfs inventory"
                }
                4 => {
                    manifest.rootfs[1].checkpoint_key = "a".into();
                    "checkpoint contains invalid rootfs inventory"
                }
                5 => {
                    manifest.rootfs[0].device_id.clear();
                    "checkpoint contains invalid rootfs inventory"
                }
                6 => {
                    manifest.rootfs[0]
                        .immutable_lower
                        .as_mut()
                        .unwrap()
                        .device_id
                        .clear();
                    "checkpoint contains invalid rootfs inventory"
                }
                7 => {
                    request.tasks[0].checkpoint_key = "missing".into();
                    "restore task key missing is absent"
                }
                8 => {
                    manifest.rootfs[0].checkpoint_key = "missing".into();
                    "restore task key a has no rootfs"
                }
                9 => {
                    request.tasks[0].task_id.clear();
                    "invalid or duplicate restored task id"
                }
                10 => {
                    request.tasks[1].task_id = request.tasks[0].task_id.clone();
                    "invalid or duplicate restored task id"
                }
                11 => {
                    manifest.rootfs[0].device_id.clear();
                    request.tasks[0].checkpoint_key = "missing".into();
                    "checkpoint contains invalid rootfs inventory"
                }
                12 => {
                    request.tasks[0].checkpoint_key = "missing".into();
                    request.tasks[0].task_id.clear();
                    "restore task key missing is absent"
                }
                13 => {
                    manifest.rootfs[1].checkpoint_key = "a".into();
                    manifest.rootfs[0].device_id.clear();
                    "checkpoint contains invalid rootfs inventory"
                }
                14 => {
                    request.tasks.pop();
                    manifest.tasks[1].checkpoint_key = "a".into();
                    "restore task inventory does not match checkpoint"
                }
                15 => {
                    request.tasks[1].checkpoint_key = request.tasks[0].checkpoint_key.clone();
                    "invalid or duplicate restore checkpoint key"
                }
                _ => unreachable!(),
            };
            let error = map_restore_tasks(&request, &manifest)
                .err()
                .expect("invalid inventory must be rejected");
            assert!(matches!(
                error.downcast_ref::<CommonError>(),
                Some(CommonError::InvalidSandboxOperation(_))
            ));
            assert!(
                error.to_string().contains(expected),
                "case {}: {}",
                case,
                error
            );
        }
    }
    #[tokio::test]
    async fn recovered_sandbox_removes_saved_private_artifacts() {
        use crate::sandbox::SandboxRestoreArgs;
        use crate::sandbox_persist::SandboxState;
        use persist::sandbox_persist::Persist;
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let checkpoint = directory.path().join("checkpoint");
        fs::create_dir(&checkpoint).unwrap();
        fs::write(checkpoint.join("manifest.json"), b"keep checkpoint").unwrap();
        let private = artifacts::restore_artifact_dir(&checkpoint, "0123456789abcdef").unwrap();
        fs::create_dir_all(&private).unwrap();
        fs::write(private.join("rootfs.raw"), b"private rootfs").unwrap();

        let mut saved: SandboxState = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/legacy-state.json"
        )))
        .unwrap();
        saved.restore_artifact_dir = Some(private.clone());
        let bytes = serde_json::to_vec(&saved).unwrap();
        drop(saved);

        let recovered = VirtSandbox::restore(
            SandboxRestoreArgs {
                sid: "0123456789abcdef".into(),
                toml_config: kata_types::config::TomlConfig::default(),
                sender: tokio::sync::mpsc::channel(1).0,
            },
            serde_json::from_slice(&bytes).unwrap(),
        )
        .await
        .unwrap();
        recovered.cleanup_restore_files().await.unwrap();

        assert!(!private.exists());
        assert!(checkpoint.join("manifest.json").is_file());
        assert!(recovered
            .restore_state
            .read()
            .await
            .artifact_dir()
            .is_none());
        recovered.cleanup_restore_files().await.unwrap();
    }
}

fn prepare_resources(resources: &mut Vec<ResourceConfig>, mode: StartMode, hypervisor_name: &str) {
    if matches!(mode, StartMode::Restore) && hypervisor_name != HYPERVISOR_DRAGONBALL {
        resources.retain(|r| !matches!(r, ResourceConfig::VmRootfs(_)));
    }
}

async fn await_agent_health<F, T>(health: F, ready_timeout: Duration) -> Result<()>
where
    F: std::future::Future<Output = Result<T>>,
{
    match timeout(ready_timeout, health).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(anyhow!("restored agent health probe timed out: {err}")),
    }
}

impl VirtSandbox {
    pub(in crate::sandbox) async fn resolve_start_mode(&self) -> StartMode {
        if self.restore_state.read().await.is_restoring() {
            StartMode::Restore
        } else {
            StartMode::Normal
        }
    }

    pub(in crate::sandbox) async fn prepare_restore_boot(
        &self,
        resources: &mut Vec<ResourceConfig>,
        mode: StartMode,
    ) -> Option<RestoreBoot> {
        let hypervisor_name = self
            .resource_manager
            .config()
            .await
            .runtime
            .hypervisor_name
            .clone();
        let mut boot = self.restore_state.write().await.take_boot();
        prepare_resources(resources, mode, &hypervisor_name);
        if let Some(boot) = &mut boot {
            resources.extend(
                std::mem::take(&mut boot.blocks)
                    .into_iter()
                    .map(ResourceConfig::RestoredBlock),
            );
        }

        boot
    }

    /// Validate the complete inventory after base and restored devices exist.
    pub(in crate::sandbox) async fn register_and_validate_restore_devices(
        &self,
        mode: StartMode,
    ) -> Result<()> {
        if matches!(mode, StartMode::Restore) {
            self.validate_restore_device_inventory().await?;
        }

        Ok(())
    }

    pub(crate) async fn connect_restored_agent(&self, address: &str) -> Result<()> {
        self.agent
            .start(address)
            .await
            .context(format!("connect restore agent server at {:?}", address))?;

        self.wait_agent_ready().await
    }

    /// Prove that the post-reset ttrpc accept loop can serve a request before
    /// issuing restore RPCs that must never be replayed.
    async fn wait_agent_ready(&self) -> Result<()> {
        const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        let health_result = await_agent_health(
            self.agent.check(agent::CheckRequest::new("")),
            READY_TIMEOUT,
        )
        .await;

        if let Err(health_err) = health_result {
            if let Err(disconnect_err) = self.agent.disconnect().await {
                return Err(health_err).context(format!(
                    "invalidate failed restored agent transport: {disconnect_err:#}"
                ));
            }
            return Err(health_err);
        }

        Ok(())
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use hypervisor::BlockConfigModern;
    use resource::network::{NetworkConfig, NetworkWithNetNsConfig};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn preserves_resource_order() {
        for name in [HYPERVISOR_DRAGONBALL, "qemu", "cloud-hypervisor"] {
            for mode in [StartMode::Normal, StartMode::Restore] {
                let mut resources = vec![
                    ResourceConfig::GuestExtensionImage(BlockConfigModern {
                        path_on_host: "extension".into(),
                        ..Default::default()
                    }),
                    ResourceConfig::VmRootfs(BlockConfigModern::default()),
                    ResourceConfig::Network(NetworkConfig::NetNs(NetworkWithNetNsConfig {
                        network_model: "tcfilter".into(),
                        netns_path: String::new(),
                        queues: 1,
                        network_created: false,
                    })),
                    ResourceConfig::InitData(BlockConfigModern {
                        path_on_host: "initdata".into(),
                        ..Default::default()
                    }),
                ];

                prepare_resources(&mut resources, mode, name);

                let keep_rootfs = mode != StartMode::Restore || name == HYPERVISOR_DRAGONBALL;
                assert_eq!(resources.len(), if keep_rootfs { 4 } else { 3 });
                assert!(
                    matches!(&resources[0], ResourceConfig::GuestExtensionImage(c)
                        if c.path_on_host == "extension")
                );
                if keep_rootfs {
                    assert!(matches!(&resources[1], ResourceConfig::VmRootfs(_)));
                }
                let network = if keep_rootfs { 2 } else { 1 };
                assert!(matches!(
                    &resources[network],
                    ResourceConfig::Network { .. }
                ));
                assert!(
                    matches!(&resources[network + 1], ResourceConfig::InitData(c)
                        if c.path_on_host == "initdata")
                );
            }
        }
    }

    #[tokio::test]
    async fn delayed_health_uses_one_request() {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let health = async move {
            observed.fetch_add(1, Ordering::AcqRel);
            tokio::time::sleep(Duration::from_millis(400)).await;
            Ok::<_, anyhow::Error>(())
        };

        await_agent_health(health, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::Acquire), 1);
    }
}
