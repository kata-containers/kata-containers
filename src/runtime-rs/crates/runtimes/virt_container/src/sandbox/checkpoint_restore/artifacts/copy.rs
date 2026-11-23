// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

//! Private copies of checkpoint files for restore.

use anyhow::{anyhow, Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::PathBuf,
    time::Instant,
};
use tokio::task::JoinSet;

use super::{run_artifact_worker, ArtifactError, RESTORE_ARTIFACT_CONCURRENCY};

pub(in crate::sandbox::checkpoint_restore) struct RestoreArtifact {
    pub(super) source: PathBuf,
    pub(super) destination: PathBuf,
    pub(super) expected_size: u64,
    pub(super) mode: u32,
    pub(super) sync_destination: bool,
}

impl RestoreArtifact {
    async fn restore(self) -> Result<()> {
        run_artifact_worker(move || self.restore_private()).await
    }

    fn restore_private(&self) -> Result<()> {
        let started = Instant::now();

        let result = (|| -> Result<()> {
            let reflink = self.try_clone()?;
            if reflink {
                debug!(
                    sl!(),
                    "clone private restore artifact with reflink";
                    "source" => self.source.display().to_string(),
                    "destination" => self.destination.display().to_string(),
                );
            } else {
                debug!(
                    sl!(),
                    "copy private restore artifact";
                    "source" => self.source.display().to_string(),
                    "destination" => self.destination.display().to_string(),
                );
                self.copy_private(!self.sync_destination)?;
            }

            debug!(
                sl!(),
                "created private restore artifact";
                "source" => self.source.display().to_string(),
                "destination" => self.destination.display().to_string(),
                "elapsed_us" => started.elapsed().as_micros(),
            );

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&self.destination);
        }

        result
    }

    fn try_clone(&self) -> Result<bool> {
        const FICLONE: u64 = 0x4004_9409;

        let source_file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.source)
            .context(format!(
                "open checkpoint artifact {}",
                self.source.display()
            ))?;

        let destination_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(self.mode)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.destination)
            .context(format!(
                "create private restore artifact {}",
                self.destination.display()
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
                fs::remove_file(&self.destination)?;

                return Ok(false);
            }

            return Err(clone_error).context("clone checkpoint artifact");
        }

        let metadata = destination_file.metadata()?;
        if !metadata.is_file() || metadata.len() != self.expected_size {
            return Err(ArtifactError::Invalid(format!(
                "checkpoint artifact {} has an invalid size",
                self.source.display()
            ))
            .into());
        }

        destination_file.set_permissions(fs::Permissions::from_mode(self.mode))?;
        destination_file.sync_all()?;

        Ok(true)
    }

    pub(super) fn copy_private(&self, preserve_holes: bool) -> Result<()> {
        let mut source_file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.source)
            .context(format!(
                "open checkpoint artifact {}",
                self.source.display()
            ))?;

        if !source_file.metadata()?.is_file() {
            return Err(ArtifactError::Invalid(format!(
                "checkpoint artifact {} is not a regular file",
                self.source.display()
            ))
            .into());
        }

        let mut destination_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(self.mode)
            .open(&self.destination)
            .context(format!(
                "create private restore artifact {}",
                self.destination.display()
            ))?;

        let copy_result = (|| -> Result<()> {
            let mut size = 0_u64;
            let mut buffer = vec![0_u8; 1024 * 1024];

            loop {
                let count = source_file.read(&mut buffer).context(format!(
                    "read checkpoint artifact {}",
                    self.source.display()
                ))?;
                if count == 0 {
                    break;
                }

                if preserve_holes && buffer[..count].iter().all(|byte| *byte == 0) {
                    destination_file
                        .seek(SeekFrom::Current(count as i64))
                        .context(format!(
                            "seek over zero range in private restore artifact {}",
                            self.destination.display()
                        ))?;
                } else {
                    destination_file
                        .write_all(&buffer[..count])
                        .context(format!(
                            "write private restore artifact {}",
                            self.destination.display()
                        ))?;
                }

                size += count as u64;
            }

            if size != self.expected_size {
                return Err(ArtifactError::Invalid(format!(
                    "checkpoint artifact {} has an invalid size",
                    self.source.display()
                ))
                .into());
            }

            destination_file.set_len(size)?;

            if self.sync_destination {
                destination_file.sync_all()?;
            }

            Ok(())
        })();

        if copy_result.is_err() {
            let _ = fs::remove_file(&self.destination);
        }

        copy_result
    }
}

pub(in crate::sandbox::checkpoint_restore) async fn restore_private_artifacts(
    artifacts: Vec<RestoreArtifact>,
) -> Result<()> {
    let mut artifacts = artifacts.into_iter();
    let mut workers = JoinSet::new();
    for artifact in artifacts.by_ref().take(RESTORE_ARTIFACT_CONCURRENCY) {
        workers.spawn(artifact.restore());
    }

    let mut first_error = None;
    while let Some(result) = workers.join_next().await {
        match result {
            Ok(Ok(())) if first_error.is_none() => {
                if let Some(artifact) = artifacts.next() {
                    workers.spawn(artifact.restore());
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
        let size = fs::metadata(&source).unwrap().len();

        RestoreArtifact {
            source,
            destination: destination.clone(),
            expected_size: size,
            mode: 0o600,
            sync_destination: false,
        }
        .copy_private(true)
        .unwrap();

        assert_eq!(fs::read(&destination).unwrap(), contents);
        assert_eq!(fs::metadata(destination).unwrap().len(), 2 * 1024 * 1024);
    }

    #[test]
    fn ephemeral_restore_copy_removes_destination_after_size_failure() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("rootfs-base.raw");
        fs::write(&source, b"rootfs contents").unwrap();
        let size = fs::metadata(&source).unwrap().len();

        let size_mismatch = directory.path().join("rootfs-size-mismatch.raw");
        assert!(RestoreArtifact {
            source: source.clone(),
            destination: size_mismatch.clone(),
            expected_size: size - 1,
            mode: 0o600,
            sync_destination: false,
        }
        .copy_private(true)
        .is_err());
        assert!(!size_mismatch.exists());
    }

    #[test]
    fn private_restore_checks_target_size() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("state");
        let destination = directory.path().join("restored-state");
        fs::write(&source, b"state").unwrap();

        assert!(RestoreArtifact {
            source,
            destination: destination.clone(),
            expected_size: 4,
            mode: 0o600,
            sync_destination: true,
        }
        .restore_private()
        .is_err());
        assert!(!destination.exists());
    }
}
