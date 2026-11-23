// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use common::error::Error as CommonError;
use hypervisor::{VmResourceTopology, VmSaveResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

use super::super::{file_integrity, manifest::WORKLOAD_CHECKPOINT_STATE};
use super::{clone_verified_artifacts, ArtifactError, RestoreArtifact};

pub(super) const VM_ARTIFACT_RAW_FORMAT: &str = "raw";

/// Metadata already computed while a backend encoded a native artifact.
pub(super) struct EncodedArtifact {
    pub format: String,
    pub logical_size: u64,
    pub encoded_size: u64,
    pub sha256: String,
}

/// Metadata for one VM checkpoint artifact.
/// Manifest paths include `vm/`; `from_manifest()` converts them to native file names.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::sandbox::checkpoint_restore) struct VmCheckpointArtifact {
    pub(in crate::sandbox::checkpoint_restore) file: String,
    pub(in crate::sandbox::checkpoint_restore) format: String,
    pub(in crate::sandbox::checkpoint_restore) logical_size: u64,
    pub(in crate::sandbox::checkpoint_restore) encoded_size: u64,
    pub(in crate::sandbox::checkpoint_restore) sha256: String,
}

impl VmCheckpointArtifact {
    /// Convert checkpoint manifest paths into native VM artifact names.
    pub(in crate::sandbox::checkpoint_restore) fn from_manifest(
        artifacts: &[Self],
    ) -> Result<Vec<Self>> {
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

    /// Describe a verified private copy; the worker performs the actual I/O.
    fn restore_copy(&self, source: PathBuf, destination: PathBuf) -> RestoreArtifact {
        RestoreArtifact {
            source,
            destination,
            expected_size: self.encoded_size,
            expected_sha256: self.sha256.clone(),
            mode: 0o600,
            sync_destination: true,
        }
    }

    pub(in crate::sandbox::checkpoint_restore) fn inspect_source(
        &self,
        source_dir: &Path,
    ) -> Result<PathBuf> {
        let source = source_dir.join(&self.file);
        let metadata = fs::symlink_metadata(&source)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.permissions().mode() & 0o222 != 0
            || metadata.len() != self.encoded_size
        {
            return Err(ArtifactError::Invalid(format!(
                "VM checkpoint artifact {} is invalid",
                source.display()
            ))
            .into());
        }

        Ok(source)
    }

    fn collect_vm_artifacts(
        state_dir: &Path,
        encoded: &HashMap<PathBuf, EncodedArtifact>,
    ) -> Result<Vec<Self>> {
        let mut entries = fs::read_dir(state_dir)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        if entries.is_empty() {
            return Err(ArtifactError::Invalid(
                "hypervisor produced no VM checkpoint artifacts".into(),
            )
            .into());
        }

        let mut artifacts = Vec::with_capacity(entries.len());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ArtifactError::Invalid(format!(
                    "VM checkpoint artifact {} is not a regular file",
                    path.display()
                ))
                .into());
            }

            fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;

            let file = entry.file_name().to_string_lossy().to_string();
            let artifact = if let Some(encoded) = encoded.get(&path) {
                Self {
                    file,
                    format: encoded.format.clone(),
                    logical_size: encoded.logical_size,
                    encoded_size: encoded.encoded_size,
                    sha256: encoded.sha256.clone(),
                }
            } else {
                let (size, sha256) = file_integrity(&path)?;
                Self {
                    file,
                    format: VM_ARTIFACT_RAW_FORMAT.into(),
                    logical_size: size,
                    encoded_size: size,
                    sha256,
                }
            };
            artifacts.push(artifact);
        }

        fs::File::open(state_dir)?.sync_all()?;

        Ok(artifacts)
    }
}

/// A native VM artifact inventory with its backend and saved resource topology.
/// Own the converted entries so restore workers can consume them without another clone.
pub(in crate::sandbox::checkpoint_restore) struct VmArtifacts<'a> {
    pub(in crate::sandbox::checkpoint_restore) hypervisor_name: &'a str,
    pub(in crate::sandbox::checkpoint_restore) artifacts: Vec<VmCheckpointArtifact>,
    pub(in crate::sandbox::checkpoint_restore) topology: VmResourceTopology,
}

impl VmArtifacts<'_> {
    pub(in crate::sandbox::checkpoint_restore) async fn validate(&self) -> Result<()> {
        self.validate_inventory()
    }
}

impl VmArtifacts<'_> {
    fn validate_inventory(&self) -> Result<()> {
        let hypervisor_name = self.hypervisor_name;
        let artifacts = &self.artifacts;
        let _topology = self.topology;
        if hypervisor_name == hypervisor::HYPERVISOR_DRAGONBALL {
            #[cfg(all(
                feature = "dragonball",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            return super::sparse_memory::validate_artifacts(_topology, artifacts)
                .map_err(map_artifact_error);

            #[cfg(not(all(
                feature = "dragonball",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )))]
            return Err(CommonError::SandboxOperationUnsupported(
                "Dragonball support is not enabled".into(),
            )
            .into());
        }

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

        Self::validate_raw_artifacts(artifacts).map_err(map_artifact_error)
    }

    pub(in crate::sandbox::checkpoint_restore) async fn prepare_restore(
        self,
        checkpoint_root: &Path,
        restore_root: &Path,
    ) -> Result<PathBuf> {
        self.validate().await?;
        let artifacts = self.artifacts;

        let restore_dir = restore_root.join(WORKLOAD_CHECKPOINT_STATE);
        let source_dir = checkpoint_root.join(WORKLOAD_CHECKPOINT_STATE);
        #[cfg(all(
            feature = "dragonball",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        if self.hypervisor_name == hypervisor::HYPERVISOR_DRAGONBALL {
            fs::create_dir(&restore_dir)?;

            let mut copies = Vec::new();
            for artifact in &artifacts {
                let source = artifact
                    .inspect_source(&source_dir)
                    .map_err(map_artifact_error)?;
                if artifact.format == VM_ARTIFACT_RAW_FORMAT {
                    copies.push(artifact.restore_copy(source, restore_dir.join(&artifact.file)));
                }
            }

            clone_verified_artifacts(copies)
                .await
                .map_err(map_artifact_error)?;

            super::sparse_memory::prepare_restore_memory(
                source_dir,
                restore_dir.clone(),
                artifacts,
            )
            .await
            .map_err(map_artifact_error)?;

            return Ok(restore_dir);
        }

        Self::prepare_raw_restore(source_dir, restore_dir.clone(), artifacts)
            .await
            .map_err(map_artifact_error)?;

        Ok(restore_dir)
    }

    /// Check names, uniqueness, sizes, and digests without interpreting backend formats.
    pub(super) fn validate_metadata(artifacts: &[VmCheckpointArtifact]) -> Result<()> {
        if artifacts.is_empty() {
            return Err(
                ArtifactError::Invalid("checkpoint has no VM state artifacts".into()).into(),
            );
        }

        let mut names = HashSet::new();
        for artifact in artifacts {
            let mut path = Path::new(&artifact.file).components();
            if !matches!(path.next(), Some(std::path::Component::Normal(_)))
                || path.next().is_some()
            {
                return Err(
                    ArtifactError::Invalid("invalid VM checkpoint artifact path".into()).into(),
                );
            }

            if !names.insert(&artifact.file) {
                return Err(
                    ArtifactError::Invalid("duplicate VM checkpoint artifact".into()).into(),
                );
            }

            if artifact.logical_size == 0
                || artifact.encoded_size == 0
                || artifact.sha256.len() != 64
                || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(ArtifactError::Invalid(
                    "checkpoint VM artifact has invalid size or SHA-256".into(),
                )
                .into());
            }
        }

        Ok(())
    }

    fn validate_raw_artifacts(artifacts: &[VmCheckpointArtifact]) -> Result<()> {
        Self::validate_metadata(artifacts)?;
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

    async fn prepare_raw_restore(
        source_dir: PathBuf,
        destination_dir: PathBuf,
        artifacts: Vec<VmCheckpointArtifact>,
    ) -> Result<()> {
        Self::validate_raw_artifacts(&artifacts)?;

        fs::create_dir(&destination_dir)?;

        let copies = artifacts
            .iter()
            .map(|artifact| {
                let source = artifact.inspect_source(&source_dir)?;
                Ok(artifact.restore_copy(source, destination_dir.join(&artifact.file)))
            })
            .collect::<Result<Vec<_>>>()?;

        clone_verified_artifacts(copies).await?;
        fs::File::open(destination_dir)?.sync_all()?;

        Ok(())
    }

    fn validate_raw_save(saved: &VmSaveResult) -> Result<()> {
        if saved.transient_memory.is_some()
            || saved.delta_ranges.is_some()
            || saved.memory_base.is_some()
        {
            return Err(anyhow!("raw VM save returned a native memory capture"));
        }

        Ok(())
    }
}

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

pub(in crate::sandbox::checkpoint_restore) async fn finalize_checkpoint_artifacts(
    hypervisor_name: &str,
    saved: VmSaveResult,
    state_dir: PathBuf,
) -> Result<Vec<VmCheckpointArtifact>> {
    let hypervisor_name = hypervisor_name.to_owned();
    tokio::task::spawn_blocking(move || {
        if hypervisor_name == hypervisor::HYPERVISOR_DRAGONBALL {
            #[cfg(all(
                feature = "dragonball",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            {
                let encoded = super::sparse_memory::finalize_checkpoint_memory(saved, &state_dir)?;
                return VmCheckpointArtifact::collect_vm_artifacts(&state_dir, &encoded);
            }
            #[cfg(not(all(
                feature = "dragonball",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )))]
            return Err(CommonError::SandboxOperationUnsupported(
                "Dragonball support is not enabled".into(),
            )
            .into());
        }

        VmArtifacts::validate_raw_save(&saved)?;

        VmCheckpointArtifact::collect_vm_artifacts(&state_dir, &HashMap::new())
    })
    .await
    .context("checkpoint VM artifact worker failed")?
    .map_err(map_artifact_error)
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
            let inventory = VmArtifacts {
                hypervisor_name: backend,
                artifacts,
                topology,
            };
            inventory.validate().await.unwrap();
            artifacts = inventory.artifacts;
            for artifact in &mut artifacts {
                artifact.file = format!("vm/{}", artifact.file);
            }
            let restore_root = directory.path().join("restore");
            fs::create_dir(&restore_root).unwrap();
            let restored = async {
                VmArtifacts {
                    hypervisor_name: backend,
                    artifacts: VmCheckpointArtifact::from_manifest(&artifacts)?,
                    topology,
                }
                .prepare_restore(&checkpoint_root, &restore_root)
                .await
            }
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
        let original = VmCheckpointArtifact {
            file: "vm/memory".into(),
            format: "native-format".into(),
            logical_size: 128,
            encoded_size: 64,
            sha256: "a".repeat(64),
        };
        let native = VmCheckpointArtifact::from_manifest(std::slice::from_ref(&original)).unwrap();
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
            assert!(VmCheckpointArtifact::from_manifest(&[invalid]).is_err());
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
        assert!(VmArtifacts::validate_raw_artifacts(std::slice::from_ref(&artifact)).is_err());
        artifact.format = VM_ARTIFACT_RAW_FORMAT.into();
        VmArtifacts::validate_raw_artifacts(std::slice::from_ref(&artifact)).unwrap();
        artifact.encoded_size -= 1;
        assert!(VmArtifacts::validate_raw_artifacts(&[artifact]).is_err());
    }
}
