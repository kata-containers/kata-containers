// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::{file_integrity, run_artifact_worker, ArtifactError, RESTORE_ARTIFACT_CONCURRENCY};
use anyhow::{anyhow, Context, Result};
use ring::digest::{Context as Sha256Context, SHA256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::task::JoinSet;

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

pub(in crate::sandbox::checkpoint_restore) async fn clone_verified_artifacts(
    artifacts: Vec<RestoreArtifact>,
) -> Result<()> {
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

pub(in crate::sandbox::checkpoint_restore) struct RestoreArtifact {
    pub(super) source: PathBuf,
    pub(super) destination: PathBuf,
    pub(super) expected_size: u64,
    pub(super) expected_sha256: String,
    pub(super) mode: u32,
    pub(super) sync_destination: bool,
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
}
