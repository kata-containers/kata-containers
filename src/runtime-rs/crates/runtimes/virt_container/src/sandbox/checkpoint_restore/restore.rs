// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    ffi::OsStr,
    fs::{self, DirBuilder},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use common::{
    error::Error as CommonError,
    types::{PrepareRestoreOptions, RestoreSandboxInfo, RestoreSandboxRequest},
};

use crate::sandbox::VirtSandbox;

use super::{
    artifacts::{rootfs, vm, RESTORE_ARTIFACT_CONCURRENCY, RESTORE_ARTIFACT_SEMAPHORE},
    manifest::CheckpointManifest,
    validation::{RestoreBindingValidation, RestoreManifestValidation, RuntimeValidation},
    PendingRestore,
};
use crate::sandbox::startup::StartMode;
use hypervisor::{VmResourceTopology, VmRestoreOptions, HYPERVISOR_DRAGONBALL};
use persist::sandbox_persist::Persist;
use resource::ResourceConfig;
use tokio::time::timeout;

fn restore_artifact_dir(checkpoint: &Path, sandbox_id: &str) -> Result<PathBuf> {
    let parent = checkpoint.parent().ok_or_else(|| {
        CommonError::InvalidSandboxOperation("checkpoint path has no parent directory".to_string())
    })?;

    let sandbox_path = Path::new(sandbox_id);
    let mut components = sandbox_path.components();
    if !matches!(
        components.next(),
        Some(std::path::Component::Normal(component))
            if component == OsStr::new(sandbox_id)
    ) || components.next().is_some()
    {
        return Err(CommonError::InvalidSandboxOperation(
            "sandbox id is not a safe restore path component".to_string(),
        )
        .into());
    }

    Ok(parent.join(".kata-restore").join(sandbox_path))
}

fn remove_private_restore_dir(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).context("inspect private restore artifacts"),
    };

    if metadata.file_type().is_symlink() {
        fs::remove_file(path).context("remove private restore artifact symlink")
    } else if metadata.is_dir() {
        fs::remove_dir_all(path).context("remove private restore artifacts")
    } else {
        Err(CommonError::InvalidSandboxOperation(format!(
            "private restore artifact path {} is not a directory",
            path.display()
        ))
        .into())
    }
}

pub(in crate::sandbox) struct RestoreBoot {
    options: VmRestoreOptions,
    blocks: Vec<hypervisor::BlockDeviceModern>,
}

impl RestoreBoot {
    /// Append restored device configs after the base configs, then return
    /// the VM restore options.
    fn into_restore_options(self, resources: &mut Vec<ResourceConfig>) -> VmRestoreOptions {
        // Append workload disks after the base devices to preserve the saved topology.
        resources.extend(self.blocks.into_iter().map(ResourceConfig::RestoredBlock));
        self.options
    }
}

impl VirtSandbox {
    pub(super) async fn create_private_restore_dir(&self, path: &Path) -> Result<()> {
        let root = path
            .parent()
            .ok_or_else(|| anyhow!("restore path has no parent directory"))?;

        match fs::symlink_metadata(root) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => {}
            Ok(_) => {
                return Err(CommonError::InvalidSandboxOperation(
                    "private restore root is not a non-symlink directory".to_string(),
                )
                .into())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                match DirBuilder::new().mode(0o700).create(root) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                        let metadata = fs::symlink_metadata(root)
                            .context("inspect concurrently created private restore root")?;
                        if metadata.file_type().is_symlink() || !metadata.is_dir() {
                            return Err(CommonError::InvalidSandboxOperation(
                                "private restore root is not a non-symlink directory".to_string(),
                            )
                            .into());
                        }
                    }
                    Err(err) => return Err(err).context("create private restore root"),
                }
            }
            Err(err) => return Err(err).context("inspect private restore root"),
        }

        match fs::symlink_metadata(path) {
            Ok(_) => return Err(anyhow!("private restore artifact directory already exists")),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err).context("inspect private restore artifact directory"),
        }

        // Record cleanup intent before creating files so a dead shim leaves
        // enough state for the delete process to remove its private artifacts.
        self.restore_state
            .write()
            .await
            .set_artifact_dir(path.to_path_buf());
        self.save()
            .await
            .context("save private restore cleanup path")?;

        DirBuilder::new()
            .mode(0o700)
            .create(path)
            .context("create private restore artifact directory")
    }

    pub(in crate::sandbox) async fn cleanup_restore_files(&self) -> Result<()> {
        let path = self.restore_state.read().await.artifact_dir();
        let Some(path) = path else {
            return Ok(());
        };

        // A timed-out Restore drops its async future, but spawn_blocking workers
        // continue. Hold every permit while removing private artifacts so cleanup
        // cannot race a copy or verification worker.
        let _artifact_permits = RESTORE_ARTIFACT_SEMAPHORE
            .acquire_many(RESTORE_ARTIFACT_CONCURRENCY as u32)
            .await
            .context("wait for private restore artifact workers")?;

        remove_private_restore_dir(&path)?;
        if let Some(root) = path.parent() {
            let _ = fs::remove_dir(root);
        }

        self.restore_state.write().await.reset();

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

    pub(in crate::sandbox) async fn prepare_restore_artifacts(
        &self,
        mode: StartMode,
    ) -> Result<()> {
        if !matches!(mode, StartMode::Restore) {
            // No restore artifact preparation is required, just skip it.
            return Ok(());
        }

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

        RuntimeValidation {
            resources: self.resource_manager.as_ref(),
        }
        .validate()
        .await?;
        let manifest = CheckpointManifest::load(&prepare.checkpoint_path)?;
        RestoreManifestValidation {
            manifest: &manifest,
            resources: self.resource_manager.as_ref(),
            template: self.template_identity.as_ref(),
        }
        .validate()
        .await?;

        let sandbox_config = self
            .sandbox_config
            .as_ref()
            .ok_or_else(|| anyhow!("sandbox config is missing"))?;

        let checkpoint_root = Path::new(&prepare.checkpoint_path);

        debug!(sl!(), "create restore rootfs overlay");

        let restore_dir = restore_artifact_dir(checkpoint_root, &sandbox_config.sandbox_id)?;
        let is_dragonball = manifest.hypervisor == hypervisor::HYPERVISOR_DRAGONBALL;

        self.create_private_restore_dir(&restore_dir).await?;

        // Runtime compatibility and private artifact preparation are independent.
        // Wait for every blocking worker to settle before returning an error so
        // rollback cannot race an in-flight copy or verification.
        let artifacts = async {
            let (blocks, state_dir) = tokio::join!(
                rootfs::RootfsRestore {
                    checkpoint_root,
                    tasks: &manifest.tasks,
                    options: &prepare.options,
                    rootfs: &manifest.rootfs,
                    restore_dir: &restore_dir,
                    sync_private_copy: !is_dragonball
                }
                .restore(),
                async {
                    vm::VmArtifacts {
                        hypervisor_name: &manifest.hypervisor,
                        artifacts: vm::VmCheckpointArtifact::from_manifest(&manifest.vm_state)?,
                        topology: hypervisor::VmResourceTopology {
                            vcpus: manifest.vm_vcpus,
                            memory_mb: manifest.vm_memory_mb,
                        },
                    }
                    .prepare_restore(checkpoint_root, &restore_dir)
                    .await
                },
            );

            let blocks = blocks.context("restore managed rootfs artifacts")?;
            let state_dir = state_dir.context("restore VM state artifacts")?;

            Ok::<_, anyhow::Error>((blocks, state_dir))
        };

        let (runtime_check, artifacts) = tokio::join!(
            self.verify_runtime_fingerprint(&manifest.runtime_fingerprint, &prepare.options),
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

        let (mappings, tasks) = RestoreBindingValidation {
            request: req,
            pending: &pending,
        }
        .validate()
        .await?;

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

    pub(in crate::sandbox) async fn prepare_vm_restore(
        &self,
        resources: &mut Vec<ResourceConfig>,
        mode: StartMode,
    ) -> (Option<VmResourceTopology>, Option<VmRestoreOptions>) {
        let hypervisor_name = self
            .resource_manager
            .config()
            .await
            .runtime
            .hypervisor_name
            .clone();

        let restore_boot = self.restore_state.write().await.take_boot();

        if matches!(mode, StartMode::Restore) && hypervisor_name != HYPERVISOR_DRAGONBALL {
            resources.retain(|r| !matches!(r, ResourceConfig::VmRootfs(_)));
        }

        let restore = restore_boot.map(|boot| boot.into_restore_options(resources));
        let restored_topology = restore.as_ref().map(|options| options.resource_topology);

        (restored_topology, restore)
    }

    pub(crate) async fn connect_restored_agent(&self, address: &str) -> Result<()> {
        self.agent
            .start(address)
            .await
            .context(format!("connect restore agent server at {:?}", address))?;

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

    pub(in crate::sandbox) async fn adopt_restored_vm_resources(
        &self,
        topology: Option<VmResourceTopology>,
    ) -> Result<()> {
        if let Some(topology) = topology {
            self.resource_manager
                .adopt_restored_vm(topology)
                .await
                .context("adopt restored VM resource topology")?;
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

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
    #[tokio::test]
    async fn recovered_sandbox_removes_saved_private_artifacts() {
        use crate::sandbox::SandboxRestoreArgs;
        use crate::sandbox_persist::SandboxState;

        let directory = tempfile::tempdir().unwrap();
        let checkpoint = directory.path().join("checkpoint");
        fs::create_dir(&checkpoint).unwrap();
        fs::write(checkpoint.join("manifest.json"), b"keep checkpoint").unwrap();
        let private = restore_artifact_dir(&checkpoint, "0123456789abcdef").unwrap();
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
