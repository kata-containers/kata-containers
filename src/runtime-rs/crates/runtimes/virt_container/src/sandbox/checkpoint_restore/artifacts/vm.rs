// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::super::manifest::{VmStateFile, WORKLOAD_CHECKPOINT_STATE};
use super::{clone_verified_artifacts, collect_vm_artifacts, RestoreArtifact};
use super::{ArtifactError, VmCheckpointArtifact, VM_ARTIFACT_RAW_FORMAT};
use anyhow::{Context, Result};
use common::error::Error as CommonError;
use hypervisor::{VmResourceTopology, VmSaveResult};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub(in crate::sandbox::checkpoint_restore) fn map_artifact_error(
    err: anyhow::Error,
) -> anyhow::Error {
    match err.downcast_ref::<ArtifactError>() {
        Some(ArtifactError::Invalid(message)) => {
            let message = message.clone();
            err.context(CommonError::InvalidSandboxOperation(message))
        }
        None => err,
    }
}

pub(in crate::sandbox::checkpoint_restore) fn native_artifacts(
    artifacts: &[VmStateFile],
) -> Result<Vec<VmCheckpointArtifact>> {
    artifacts
        .iter()
        .map(|artifact| {
            let mut path = Path::new(&artifact.file).components();
            if path.next()
                != Some(std::path::Component::Normal(std::ffi::OsStr::new(
                    WORKLOAD_CHECKPOINT_STATE,
                )))
            {
                return Err(CommonError::InvalidSandboxOperation(
                    "invalid VM checkpoint artifact path".into(),
                )
                .into());
            }

            let Some(std::path::Component::Normal(name)) = path.next() else {
                return Err(CommonError::InvalidSandboxOperation(
                    "invalid VM checkpoint artifact path".into(),
                )
                .into());
            };

            if path.next().is_some() {
                return Err(CommonError::InvalidSandboxOperation(
                    "invalid VM checkpoint artifact path".into(),
                )
                .into());
            }

            let mut artifact = artifact.clone();
            artifact.file = name.to_string_lossy().into_owned();

            Ok(artifact)
        })
        .collect()
}

pub(in crate::sandbox::checkpoint_restore) fn validate_checkpoint_artifacts(
    hypervisor_name: &str,
    artifacts: &[VmCheckpointArtifact],
    _topology: VmResourceTopology,
) -> Result<()> {
    if hypervisor_name == hypervisor::HYPERVISOR_QEMU
        && !artifacts
            .iter()
            .any(|artifact| artifact.file == "memory-topology.json")
    {
        return Err(CommonError::InvalidSandboxOperation(
            "checkpoint is missing QEMU memory topology".into(),
        )
        .into());
    }

    validate_raw_artifacts(artifacts).map_err(map_artifact_error)
}

pub(in crate::sandbox::checkpoint_restore) async fn finalize_checkpoint_artifacts(
    _hypervisor_name: &str,
    saved: VmSaveResult,
    state_dir: PathBuf,
) -> Result<Vec<VmCheckpointArtifact>> {
    tokio::task::spawn_blocking(move || {
        super::validate_raw_save(&saved)?;

        collect_vm_artifacts(&state_dir, &HashMap::new())
    })
    .await
    .context("checkpoint VM artifact worker failed")?
    .map_err(map_artifact_error)
}

pub(in crate::sandbox::checkpoint_restore) async fn restore_vm_artifacts(
    hypervisor_name: &str,
    checkpoint_root: &Path,
    artifacts: &[VmStateFile],
    restore_root: &Path,
    topology: VmResourceTopology,
) -> Result<PathBuf> {
    let artifacts = native_artifacts(artifacts)?;
    validate_checkpoint_artifacts(hypervisor_name, &artifacts, topology)?;

    let restore_dir = restore_root.join(WORKLOAD_CHECKPOINT_STATE);
    let source_dir = checkpoint_root.join(WORKLOAD_CHECKPOINT_STATE);

    prepare_raw_restore(source_dir, restore_dir.clone(), artifacts)
        .await
        .map_err(map_artifact_error)?;

    Ok(restore_dir)
}

pub(super) fn validate_raw_artifacts(artifacts: &[VmCheckpointArtifact]) -> Result<()> {
    super::validate_artifacts(artifacts)?;
    for artifact in artifacts {
        if artifact.format != VM_ARTIFACT_RAW_FORMAT
            || artifact.logical_size != artifact.encoded_size
        {
            return Err(ArtifactError::Invalid(
                "checkpoint VM artifact has an unsupported format or size".into(),
            )
            .into());
        }
    }

    Ok(())
}

pub(super) fn raw_restore_artifact(
    source: PathBuf,
    destination: PathBuf,
    artifact: &VmCheckpointArtifact,
) -> RestoreArtifact {
    RestoreArtifact {
        source,
        destination,
        expected_size: artifact.encoded_size,
        expected_sha256: artifact.sha256.clone(),
        mode: 0o600,
        sync_destination: true,
    }
}

pub(super) async fn prepare_raw_restore(
    source_dir: PathBuf,
    destination_dir: PathBuf,
    artifacts: Vec<VmCheckpointArtifact>,
) -> Result<()> {
    validate_raw_artifacts(&artifacts)?;

    fs::create_dir(&destination_dir)?;

    let copies = artifacts
        .iter()
        .map(|artifact| {
            let source = super::inspect_source(&source_dir, artifact)?;
            Ok(raw_restore_artifact(
                source,
                destination_dir.join(&artifact.file),
                artifact,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    clone_verified_artifacts(copies).await?;
    fs::File::open(destination_dir)?.sync_all()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn raw_vm_artifacts_publish_and_restore_for_qemu_and_clh() {
        for backend in [hypervisor::HYPERVISOR_QEMU, hypervisor::HYPERVISOR_NAME_CH] {
            let directory = tempfile::tempdir().unwrap();
            let checkpoint_root = directory.path().join("checkpoint");
            let state_dir = checkpoint_root.join("vm");
            fs::create_dir_all(&state_dir).unwrap();
            fs::write(state_dir.join("state"), b"native VM state").unwrap();
            if backend == hypervisor::HYPERVISOR_QEMU {
                fs::write(state_dir.join("memory-topology.json"), b"{}").unwrap();
            }
            let topology = VmResourceTopology {
                vcpus: 1,
                memory_mb: 128,
            };
            let mut artifacts =
                finalize_checkpoint_artifacts(backend, VmSaveResult::default(), state_dir.clone())
                    .await
                    .unwrap();
            validate_checkpoint_artifacts(backend, &artifacts, topology).unwrap();
            for artifact in &mut artifacts {
                artifact.file = format!("vm/{}", artifact.file);
            }
            let restore_root = directory.path().join("restore");
            fs::create_dir(&restore_root).unwrap();
            let restored = restore_vm_artifacts(
                backend,
                &checkpoint_root,
                &artifacts,
                &restore_root,
                topology,
            )
            .await
            .unwrap();
            assert_eq!(
                fs::read(restored.join("state")).unwrap(),
                b"native VM state"
            );
            if backend == hypervisor::HYPERVISOR_QEMU {
                assert_eq!(
                    fs::read(restored.join("memory-topology.json")).unwrap(),
                    b"{}"
                );
            }
        }
    }

    #[test]
    fn checkpoint_paths_map_to_native_files_without_changing_metadata() {
        let original = VmStateFile {
            file: "vm/memory".into(),
            format: "native-format".into(),
            logical_size: 128,
            encoded_size: 64,
            sha256: "a".repeat(64),
        };
        let native = native_artifacts(std::slice::from_ref(&original)).unwrap();
        assert_eq!(native[0].file, "memory");
        assert_eq!(native[0].format, original.format);
        assert_eq!(native[0].sha256, original.sha256);
        for file in [
            "memory",
            "storage/memory",
            "vm/../memory",
            "vm/sub/memory",
            "/vm/memory",
        ] {
            let mut invalid = original.clone();
            invalid.file = file.into();
            assert!(native_artifacts(&[invalid]).is_err());
        }
    }

    #[test]
    fn native_errors_keep_sandbox_status_categories() {
        let invalid = map_artifact_error(
            anyhow::Error::from(ArtifactError::Invalid("bad digest".into()))
                .context("verify native memory"),
        );
        assert!(matches!(
            invalid.downcast_ref::<CommonError>(),
            Some(CommonError::InvalidSandboxOperation(_))
        ));
        let io =
            map_artifact_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into());
        assert!(io.downcast_ref::<CommonError>().is_none());
        assert!(io.downcast_ref::<std::io::Error>().is_some());
    }

    #[test]
    fn raw_inventory_rejects_sparse_formats_and_size_mismatches() {
        let mut artifact = VmCheckpointArtifact {
            file: "state".into(),
            format: "kata-sparse-raw-v1".into(),
            logical_size: 128,
            encoded_size: 128,
            sha256: "a".repeat(64),
        };
        assert!(validate_raw_artifacts(std::slice::from_ref(&artifact)).is_err());
        artifact.format = VM_ARTIFACT_RAW_FORMAT.into();
        validate_raw_artifacts(std::slice::from_ref(&artifact)).unwrap();
        artifact.encoded_size -= 1;
        assert!(validate_raw_artifacts(&[artifact]).is_err());
    }
}
