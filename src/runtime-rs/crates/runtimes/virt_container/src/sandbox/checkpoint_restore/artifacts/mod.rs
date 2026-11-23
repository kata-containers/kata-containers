// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use crate::sandbox::VirtSandbox;
use anyhow::{anyhow, Context, Result};
use common::error::Error as CommonError;
use hypervisor::VmSaveResult;
use persist::sandbox_persist::Persist;
use ring::digest::{Context as Sha256Context, SHA256};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::{self, DirBuilder, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::{sync::Semaphore, task::JoinSet};
pub(super) mod rootfs;

#[cfg(all(
    feature = "dragonball",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(super) mod sparse_memory;
pub(super) mod vm;

pub(super) const IMMUTABLE_ROOTFS_OPTION: &str = "io.katacontainers.checkpoint.rootfs.immutable";
pub(super) const RESTORE_ARTIFACT_CONCURRENCY: usize = 2;
pub(super) static RESTORE_ARTIFACT_SEMAPHORE: Semaphore =
    Semaphore::const_new(RESTORE_ARTIFACT_CONCURRENCY);
pub(super) const VM_ARTIFACT_RAW_FORMAT: &str = "raw";

pub(super) fn restore_artifact_dir(checkpoint: &Path, sandbox_id: &str) -> Result<PathBuf> {
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

    pub(super) fn remove_private_restore_dir(path: &Path) -> Result<()> {
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

        Self::remove_private_restore_dir(&path)?;
        if let Some(root) = path.parent() {
            let _ = fs::remove_dir(root);
        }

        self.restore_state.write().await.reset();

        Ok(())
    }
}

pub(super) fn collect_vm_artifacts(
    state_dir: &Path,
    encoded: &std::collections::HashMap<PathBuf, EncodedArtifact>,
) -> Result<Vec<VmCheckpointArtifact>> {
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
            VmCheckpointArtifact {
                file,
                format: encoded.format.clone(),
                logical_size: encoded.logical_size,
                encoded_size: encoded.encoded_size,
                sha256: encoded.sha256.clone(),
            }
        } else {
            let (size, sha256) = file_integrity(&path)?;
            VmCheckpointArtifact {
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

fn copy_verified_artifact(
    source: &Path,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
    mode: u32,
    sync_destination: bool,
) -> Result<()> {
    let mut source_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)
        .context(format!("open checkpoint artifact {}", source.display()))?;

    if !source_file.metadata()?.is_file() {
        return Err(ArtifactError::Invalid(format!(
            "checkpoint artifact {} is not a regular file",
            source.display()
        ))
        .into());
    }

    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(destination)
        .context(format!(
            "create private restore artifact {}",
            destination.display()
        ))?;

    let copy_result = (|| -> Result<()> {
        let mut size = 0_u64;
        let mut hasher = Sha256Context::new(&SHA256);
        let mut buffer = vec![0_u8; 1024 * 1024];

        loop {
            let count = source_file
                .read(&mut buffer)
                .context(format!("read checkpoint artifact {}", source.display()))?;
            if count == 0 {
                break;
            }

            if !sync_destination && buffer[..count].iter().all(|byte| *byte == 0) {
                destination_file
                    .seek(SeekFrom::Current(count as i64))
                    .context(format!(
                        "seek over zero range in private restore artifact {}",
                        destination.display()
                    ))?;
            } else {
                destination_file
                    .write_all(&buffer[..count])
                    .context(format!(
                        "write private restore artifact {}",
                        destination.display()
                    ))?;
            }

            size += count as u64;
            hasher.update(&buffer[..count]);
        }

        destination_file.set_len(size)?;

        if sync_destination {
            destination_file.sync_all()?;
        }

        let digest = hasher.finish();
        let sha256: String = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        if size != expected_size || sha256 != expected_sha256 {
            return Err(ArtifactError::Invalid(format!(
                "checkpoint artifact {} failed size or SHA-256 validation",
                source.display()
            ))
            .into());
        }

        Ok(())
    })();

    if copy_result.is_err() {
        let _ = fs::remove_file(destination);
    }

    copy_result
}

fn try_clone_artifact(source: &Path, destination: &Path, mode: u32) -> Result<bool> {
    const FICLONE: u64 = 0x4004_9409;

    let source_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)
        .context(format!("open checkpoint artifact {}", source.display()))?;

    let destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(destination)
        .context(format!(
            "create private restore artifact {}",
            destination.display()
        ))?;

    let clone_result = unsafe {
        libc::ioctl(
            destination_file.as_raw_fd(),
            FICLONE as _,
            source_file.as_raw_fd(),
        )
    };

    if clone_result < 0 {
        let clone_error = std::io::Error::last_os_error();
        if matches!(
            clone_error.raw_os_error(),
            Some(code)
                if matches!(code, libc::EOPNOTSUPP | libc::EXDEV | libc::EINVAL | libc::ENOTTY)
        ) {
            drop(destination_file);
            fs::remove_file(destination)?;

            return Ok(false);
        }

        return Err(clone_error).context("clone checkpoint artifact");
    }

    destination_file.set_permissions(fs::Permissions::from_mode(mode))?;
    destination_file.sync_all()?;

    Ok(true)
}

fn clone_verified_artifact(
    source: &Path,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
    mode: u32,
    sync_destination: bool,
) -> Result<()> {
    let started = Instant::now();

    let result = (|| -> Result<()> {
        let reflink = try_clone_artifact(source, destination, mode)?;
        if reflink {
            debug!(
                sl!(),
                "clone verified restore artifact with reflink";
                "source" => source.display().to_string(),
                "destination" => destination.display().to_string(),
            );
            let (size, sha256) = file_integrity(destination)?;
            if size != expected_size || sha256 != expected_sha256 {
                return Err(ArtifactError::Invalid(format!(
                    "checkpoint artifact {} failed size or SHA-256 validation",
                    source.display()
                ))
                .into());
            }
        } else {
            debug!(
                sl!(),
                "copy verified restore artifact";
                "source" => source.display().to_string(),
                "destination" => destination.display().to_string(),
            );
            copy_verified_artifact(
                source,
                destination,
                expected_size,
                expected_sha256,
                mode,
                sync_destination,
            )?;
        }

        debug!(
            sl!(),
            "created verified private restore artifact";
            "source" => source.display().to_string(),
            "destination" => destination.display().to_string(),
            "elapsed_us" => started.elapsed().as_micros(),
        );

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(destination);
    }

    result
}

async fn clone_verified_artifact_limited(artifact: RestoreArtifact) -> Result<()> {
    run_artifact_worker(move || {
        clone_verified_artifact(
            &artifact.source,
            &artifact.destination,
            artifact.expected_size,
            &artifact.expected_sha256,
            artifact.mode,
            artifact.sync_destination,
        )
    })
    .await
}

pub(super) async fn clone_verified_artifacts(artifacts: Vec<RestoreArtifact>) -> Result<()> {
    let mut artifacts = artifacts.into_iter();
    let mut workers = JoinSet::new();
    for artifact in artifacts.by_ref().take(RESTORE_ARTIFACT_CONCURRENCY) {
        workers.spawn(clone_verified_artifact_limited(artifact));
    }

    let mut first_error = None;
    while let Some(result) = workers.join_next().await {
        match result {
            Ok(Ok(())) if first_error.is_none() => {
                if let Some(artifact) = artifacts.next() {
                    workers.spawn(clone_verified_artifact_limited(artifact));
                }
            }
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                first_error.get_or_insert(err);
            }
            Err(err) => {
                first_error.get_or_insert_with(|| {
                    anyhow!("private restore artifact scheduler failed: {err}")
                });
            }
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

pub(super) struct RestoreArtifact {
    source: PathBuf,
    destination: PathBuf,
    expected_size: u64,
    expected_sha256: String,
    mode: u32,
    sync_destination: bool,
}

/// A native VM artifact, with a single file name relative to the VM directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VmCheckpointArtifact {
    pub file: String,
    pub format: String,
    pub logical_size: u64,
    pub encoded_size: u64,
    pub sha256: String,
}

/// Check generic native artifact metadata without interpreting backend formats.
pub(super) fn validate_artifacts(artifacts: &[VmCheckpointArtifact]) -> Result<()> {
    if artifacts.is_empty() {
        return Err(ArtifactError::Invalid("checkpoint has no VM state artifacts".into()).into());
    }
    let mut names = HashSet::new();
    for artifact in artifacts {
        let mut path = Path::new(&artifact.file).components();
        if !matches!(path.next(), Some(std::path::Component::Normal(_))) || path.next().is_some() {
            return Err(
                ArtifactError::Invalid("invalid VM checkpoint artifact path".into()).into(),
            );
        }

        if !names.insert(&artifact.file) {
            return Err(ArtifactError::Invalid("duplicate VM checkpoint artifact".into()).into());
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

pub(super) fn inspect_source(
    source_dir: &Path,
    artifact: &VmCheckpointArtifact,
) -> Result<PathBuf> {
    let source = source_dir.join(&artifact.file);
    let metadata = fs::symlink_metadata(&source)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.permissions().mode() & 0o222 != 0
        || metadata.len() != artifact.encoded_size
    {
        return Err(ArtifactError::Invalid(format!(
            "VM checkpoint artifact {} is invalid",
            source.display()
        ))
        .into());
    }

    Ok(source)
}

/// Metadata already computed while a backend encoded a native artifact.
pub(super) struct EncodedArtifact {
    pub format: String,
    pub logical_size: u64,
    pub encoded_size: u64,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ArtifactError {
    #[error("{0}")]
    Invalid(String),
}

pub(super) fn file_integrity(path: &Path) -> Result<(u64, String)> {
    let file =
        fs::File::open(path).context(format!("open checkpoint artifact {}", path.display()))?;
    let size = file
        .metadata()
        .context(format!("inspect checkpoint artifact {}", path.display()))?
        .len();

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256Context::new(&SHA256);

    // This helper runs from Tokio worker tasks whose stacks are small.
    // Keep the large I/O buffer on the heap to avoid aborting a deeply
    // nested Restore call with a worker-stack overflow.
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .context(format!("read checkpoint artifact {}", path.display()))?;
        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    let digest = hasher.finish();
    let sha256 = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok((size, sha256))
}

pub(super) async fn run_artifact_worker<F>(worker: F) -> Result<()>
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

pub(super) fn validate_raw_save(saved: &VmSaveResult) -> Result<()> {
    if saved.transient_memory.is_some()
        || saved.delta_ranges.is_some()
        || saved.memory_base.is_some()
    {
        return Err(anyhow!("raw VM save returned a native memory capture"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_restore_copy_supports_ephemeral_destination() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("rootfs-base.raw");
        let destination = directory.path().join("rootfs.raw");
        let mut contents = vec![0_u8; 2 * 1024 * 1024];
        contents[..15].copy_from_slice(b"rootfs contents");
        fs::write(&source, &contents).unwrap();
        let (size, sha256) = file_integrity(&source).unwrap();

        copy_verified_artifact(&source, &destination, size, &sha256, 0o600, false).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), contents);
        assert_eq!(fs::metadata(destination).unwrap().len(), 2 * 1024 * 1024);
    }

    #[test]
    fn ephemeral_restore_copy_removes_destination_after_verification_failure() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("rootfs-base.raw");
        fs::write(&source, b"rootfs contents").unwrap();
        let (size, sha256) = file_integrity(&source).unwrap();

        let size_mismatch = directory.path().join("rootfs-size-mismatch.raw");
        assert!(
            copy_verified_artifact(&source, &size_mismatch, size - 1, &sha256, 0o600, false)
                .is_err()
        );
        assert!(!size_mismatch.exists());

        let digest_mismatch = directory.path().join("rootfs-digest-mismatch.raw");
        assert!(
            copy_verified_artifact(&source, &digest_mismatch, size, "invalid", 0o600, false)
                .is_err()
        );
        assert!(!digest_mismatch.exists());
    }

    use std::sync::mpsc;
    use std::time::Duration;

    use tokio::sync::oneshot;
    use tokio::time::timeout;

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

    #[test]
    fn native_inventory_rejects_paths_duplicates_and_unknown_fields() {
        let artifact = VmCheckpointArtifact {
            file: "memory".into(),
            format: VM_ARTIFACT_RAW_FORMAT.into(),
            logical_size: 128,
            encoded_size: 128,
            sha256: "a".repeat(64),
        };
        for file in ["../memory", "/memory", "vm/memory", ".", ""] {
            let mut invalid = artifact.clone();
            invalid.file = file.into();
            assert!(validate_artifacts(&[invalid]).is_err());
        }
        assert!(validate_artifacts(&[artifact.clone(), artifact.clone()]).is_err());
        let mut value = serde_json::to_value(&artifact).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), serde_json::json!(true));
        assert!(serde_json::from_value::<VmCheckpointArtifact>(value).is_err());
    }

    #[test]
    fn template_save_rejects_transient_memory() {
        VmSaveResult::default().validate_template().unwrap();
        let saved = VmSaveResult {
            transient_memory: Some(tempfile::tempfile().unwrap()),
            ..Default::default()
        };
        assert!(saved.validate_template().is_err());
    }
}
