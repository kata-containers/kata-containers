// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

//! Sparse-memory snapshot artifacts using the Dragonball memory codecs.

use super::{
    file_integrity, run_artifact_worker, ArtifactError, EncodedArtifact, VmCheckpointArtifact,
    VM_ARTIFACT_RAW_FORMAT,
};
use anyhow::{anyhow, Context, Result};
use dragonball::snapshot::memory::{
    copy_flat_sparse_file, encode_memory_overlay, memory_overlay_size, merge_flat_sparse_file,
    verify_flat_sparse_file, CodecError, SparseArtifact, SparseEncodeStats, FLAT_SPARSE_FORMAT,
    OVERLAY_FORMAT,
};
use hypervisor::{VmMemoryRange, VmResourceTopology, VmSaveResult};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
use std::{
    io::Write,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
};

fn map_memory_error(error: anyhow::Error) -> anyhow::Error {
    match error.downcast_ref::<CodecError>() {
        Some(CodecError::Invalid(message)) => {
            let message = message.clone();
            error.context(ArtifactError::Invalid(message))
        }
        None => error,
    }
}

pub(super) struct SparseMeta<'a> {
    pub(super) encoded_size: u64,
    pub(super) logical_size: u64,
    pub(super) sha256: &'a str,
}

pub(super) fn validate_artifacts(
    topology: VmResourceTopology,
    artifacts: &[VmCheckpointArtifact],
) -> Result<()> {
    super::validate_artifacts(artifacts)?;

    let expected_size = u64::from(topology.memory_mb) * 1024 * 1024;
    let mut memory = false;
    let mut state = false;
    let mut overlay = false;

    for artifact in artifacts {
        match (artifact.file.as_str(), artifact.format.as_str()) {
            ("memory", FLAT_SPARSE_FORMAT)
                if artifact.logical_size == expected_size
                    && artifact.encoded_size == expected_size =>
            {
                memory = true
            }
            ("state", VM_ARTIFACT_RAW_FORMAT) if artifact.logical_size == artifact.encoded_size => {
                state = true
            }
            ("memory.overlay", OVERLAY_FORMAT)
                if artifact.logical_size == expected_size
                    && artifact.encoded_size <= expected_size =>
            {
                overlay = true
            }
            _ => {
                return Err(ArtifactError::Invalid(
                    "Dragonball checkpoint has an unexpected VM artifact".into(),
                )
                .into())
            }
        }
    }

    if !memory || !state || artifacts.len() != if overlay { 3 } else { 2 } {
        return Err(ArtifactError::Invalid(
            "Dragonball checkpoint requires memory and state artifacts".into(),
        )
        .into());
    }

    Ok(())
}

/// Finalize captured memory on the caller's blocking worker after VM recovery.
pub(super) fn finalize_checkpoint_memory(
    mut saved: VmSaveResult,
    state_dir: &Path,
) -> Result<HashMap<PathBuf, EncodedArtifact>> {
    let mut source = saved
        .transient_memory
        .take()
        .ok_or_else(|| anyhow!("Dragonball workload save returned no transient memory"))?;
    let started = Instant::now();
    let sparse = checkpoint_memory(
        &mut source,
        saved.delta_ranges.as_deref(),
        saved.memory_base.as_deref(),
        state_dir,
    )
    .map_err(map_memory_error)?;
    drop(source);
    fs::File::open(state_dir)?.sync_all()?;
    debug!(sl!(), "published Dragonball VM memory artifacts";
        "artifact_count" => sparse.len(),
        "encoded_bytes" => sparse.values().map(|(_, value)| value.encoded_size).sum::<u64>(),
        "written_bytes" => sparse.values().map(|(_, value)| value.stats.written_bytes).sum::<u64>(),
        "elapsed_us" => started.elapsed().as_micros()
    );

    Ok(sparse
        .into_iter()
        .map(|(path, (format, value))| {
            (
                path,
                EncodedArtifact {
                    format,
                    logical_size: value.logical_size,
                    encoded_size: value.encoded_size,
                    sha256: value.sha256,
                },
            )
        })
        .collect())
}

/// Prepare native memory after the caller has verified/copied raw state files.
pub(super) async fn prepare_restore_memory(
    source_dir: PathBuf,
    destination_dir: PathBuf,
    artifacts: Vec<VmCheckpointArtifact>,
) -> Result<()> {
    // Inventory was checked before parallel restore work; recheck path safety here.
    super::validate_artifacts(&artifacts)?;

    let mut state = false;
    let mut memory = None;
    let mut overlay = None;

    for artifact in artifacts {
        let source = super::inspect_source(&source_dir, &artifact)?;
        let destination = destination_dir.join(&artifact.file);

        match (artifact.file.as_str(), artifact.format.as_str()) {
            ("memory", FLAT_SPARSE_FORMAT) if artifact.logical_size == artifact.encoded_size => {
                memory = Some((source, destination, artifact))
            }
            ("memory.overlay", OVERLAY_FORMAT)
                if artifact.encoded_size <= artifact.logical_size =>
            {
                overlay = Some((source, destination, artifact))
            }
            ("state", VM_ARTIFACT_RAW_FORMAT) if artifact.logical_size == artifact.encoded_size => {
                state = true;
            }
            _ => {
                return Err(ArtifactError::Invalid(
                    "Dragonball checkpoint has an unexpected VM artifact".into(),
                )
                .into())
            }
        }
    }

    let (source, destination, artifact) = memory.ok_or_else(|| {
        ArtifactError::Invalid("Dragonball checkpoint has no sparse VM memory artifact".into())
    })?;

    if !state
        || overlay
            .as_ref()
            .is_some_and(|(_, _, value)| value.logical_size != artifact.logical_size)
    {
        return Err(ArtifactError::Invalid(
            "Dragonball checkpoint requires matching memory and state artifacts".into(),
        )
        .into());
    }

    run_artifact_worker(move || {
        restore_memory(
            &source,
            &destination,
            artifact.logical_size,
            &artifact.sha256,
        )
    })
    .await?;

    if let Some((source, destination, artifact)) = overlay {
        run_artifact_worker(move || {
            restore_overlay(
                &source,
                &destination,
                SparseMeta {
                    logical_size: artifact.logical_size,
                    encoded_size: artifact.encoded_size,
                    sha256: &artifact.sha256,
                },
            )
        })
        .await?;
    }
    fs::File::open(destination_dir)?.sync_all()?;

    Ok(())
}

pub(super) fn restore_memory(
    source: &Path,
    destination: &Path,
    logical_size: u64,
    sha256: &str,
) -> Result<()> {
    let started = Instant::now();
    let verified =
        verify_flat_sparse_file(source, logical_size, sha256).map_err(map_memory_error)?;

    restore_memory_artifact(source, destination, logical_size, "memory")?;
    let mut digest = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .custom_flags(libc::O_NOFOLLOW)
        .open(destination.with_file_name(VERIFIED_MEMORY_DIGEST))
        .context("create verified Dragonball memory digest")?;

    digest.write_all(sha256.as_bytes())?;
    digest.sync_all()?;

    debug!(sl!(), "verified flat sparse VM memory artifact";
        "logical_bytes" => logical_size,
        "candidate_bytes" => verified.stats.candidate_bytes,
        "candidate_chunks" => verified.stats.candidate_chunks,
        "non_zero_chunks" => verified.non_zero_chunks,
        "elapsed_us" => started.elapsed().as_micros(),
    );

    Ok(())
}

pub(super) fn restore_overlay(
    source: &Path,
    destination: &Path,
    expected: SparseMeta<'_>,
) -> Result<()> {
    let (size, sha256) = file_integrity(source)?;
    if size != expected.encoded_size || sha256 != expected.sha256 {
        return Err(ArtifactError::Invalid(
            "Dragonball memory overlay failed integrity validation".to_string(),
        )
        .into());
    }

    if expected.logical_size == 0 {
        return Err(ArtifactError::Invalid(
            "Dragonball memory overlay has no logical size".to_string(),
        )
        .into());
    }

    restore_memory_artifact(source, destination, expected.encoded_size, "overlay")
}

fn restore_memory_artifact(source: &Path, destination: &Path, size: u64, kind: &str) -> Result<()> {
    match fs::hard_link(source, destination) {
        Ok(()) => {}
        Err(err) if err.raw_os_error() == Some(libc::EXDEV) => {
            let mut source = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(source)?;
            if source.metadata()?.len() != size {
                return Err(ArtifactError::Invalid(format!(
                    "Dragonball restore {kind} source changed size"
                ))
                .into());
            }
            copy_flat_sparse_file(&mut source, destination).with_context(|| {
                format!("copy verified Dragonball {kind} artifact across filesystems")
            })?;
            return Ok(());
        }
        Err(err) => {
            return Err(err).with_context(|| format!("link verified Dragonball {kind} artifact"))
        }
    }

    let source = fs::metadata(source)?;
    let destination = fs::metadata(destination)?;
    if source.dev() != destination.dev() || source.ino() != destination.ino() {
        return Err(ArtifactError::Invalid(format!(
            "Dragonball restore {kind} link changed inode"
        ))
        .into());
    }

    Ok(())
}

pub(super) const VERIFIED_MEMORY_DIGEST: &str = "memory.sha256";

// These inputs describe either a full memory save or a base plus dirty ranges.
enum MemorySnapshot<'a> {
    Full,
    Delta {
        base: &'a fs::File,
        ranges: &'a [VmMemoryRange],
    },
}

pub(super) fn checkpoint_memory(
    source: &mut fs::File,
    ranges: Option<&[VmMemoryRange]>,
    base: Option<&fs::File>,
    state_dir: &Path,
) -> Result<HashMap<PathBuf, (String, SparseArtifact)>> {
    let snapshot = match (ranges, base) {
        (None, None) => MemorySnapshot::Full,
        (Some(ranges), Some(base)) => MemorySnapshot::Delta { base, ranges },
        _ => {
            return Err(anyhow!(
                "Dragonball checkpoint base and delta ranges are inconsistent"
            ))
        }
    };

    let mut artifacts = HashMap::new();
    match snapshot {
        MemorySnapshot::Full => {
            let partial = state_dir.join("memory.partial");
            let destination = state_dir.join("memory");
            let encoded = copy_flat_sparse_file(source, &partial)
                .context("copy Dragonball flat sparse VM memory artifact")?;

            fs::rename(&partial, &destination)?;

            artifacts.insert(destination, (FLAT_SPARSE_FORMAT.to_string(), encoded));
        }
        MemorySnapshot::Delta { base, ranges } => {
            let destination = state_dir.join("memory");
            let partial = state_dir.join("memory.overlay.partial");
            let overlay = state_dir.join("memory.overlay");
            let base_path = fs::read_link(format!("/proc/self/fd/{}", base.as_raw_fd()))?;

            let base_sha256 = fs::read_to_string(base_path.with_file_name(VERIFIED_MEMORY_DIGEST))?;
            let base_size = base.metadata()?.len();
            if base_sha256.len() != 64
                || !base_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || base_size != source.metadata()?.len()
            {
                return Err(anyhow!("invalid verified Dragonball memory base metadata"));
            }

            let publish = (|| -> Result<()> {
                let predicted_size = memory_overlay_size(ranges)?;
                let merge = predicted_size >= base_size;
                if merge {
                    let flat_partial = state_dir.join("memory.partial");
                    let flat = merge_flat_sparse_file(base, source, ranges, &flat_partial)?;
                    fs::rename(&flat_partial, &destination)?;
                    artifacts.insert(destination.clone(), (FLAT_SPARSE_FORMAT.to_string(), flat));
                } else {
                    let base_artifact = SparseArtifact {
                        logical_size: base_size,
                        encoded_size: base_size,
                        sha256: base_sha256,
                        non_zero_chunks: 0,
                        stats: SparseEncodeStats {
                            candidate_chunks: 0,
                            candidate_bytes: 0,
                            written_bytes: 0,
                        },
                    };

                    let base_artifact = match fs::hard_link(&base_path, &destination) {
                        Ok(()) => base_artifact,
                        Err(err) if err.raw_os_error() == Some(libc::EXDEV) => {
                            let copied =
                                copy_flat_sparse_file(&mut base.try_clone()?, &destination)?;
                            if copied.sha256 != base_artifact.sha256 {
                                return Err(ArtifactError::Invalid(
                                    "Dragonball memory base changed while copying".into(),
                                )
                                .into());
                            }
                            copied
                        }
                        Err(err) => return Err(err).context("link Dragonball memory base"),
                    };
                    let encoded = encode_memory_overlay(source, ranges, &partial, &sl!())?;
                    fs::rename(&partial, &overlay)?;

                    artifacts.insert(
                        destination.clone(),
                        (FLAT_SPARSE_FORMAT.to_string(), base_artifact),
                    );
                    artifacts.insert(overlay, (OVERLAY_FORMAT.to_string(), encoded));
                }

                Ok(())
            })();

            if let Err(err) = publish {
                let _ = fs::remove_file(&destination);
                let _ = fs::remove_file(&partial);
                return Err(err).context("publish Dragonball base and overlay artifacts");
            }
        }
    }

    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::os::unix::fs::FileExt;

    #[test]
    fn restore_rejects_invalid_memory_digests() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::write(&source, b"guest-memory").unwrap();
        let invalid_digest = "0".repeat(64);
        let memory = directory.path().join("memory");
        let invalid = restore_memory(&source, &memory, 12, &invalid_digest).unwrap_err();
        assert!(matches!(
            invalid.downcast_ref::<ArtifactError>(),
            Some(ArtifactError::Invalid(_))
        ));
        assert!(!memory.exists());
        assert!(!directory.path().join(VERIFIED_MEMORY_DIGEST).exists());

        let overlay = directory.path().join("memory.overlay");
        assert!(restore_overlay(
            &source,
            &overlay,
            SparseMeta {
                logical_size: 4096,
                encoded_size: 12,
                sha256: &invalid_digest,
            }
        )
        .is_err());
        assert!(!overlay.exists());
    }

    #[test]
    fn oversized_invalid_ranges_do_not_publish_memory() {
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let directory = tempfile::tempdir().unwrap();
        let state_dir = directory.path().join("state");
        fs::create_dir(&state_dir).unwrap();
        let base_path = directory.path().join("memory");
        let base = fs::File::create(&base_path).unwrap();
        base.set_len(page * 2).unwrap();
        fs::write(
            directory.path().join(VERIFIED_MEMORY_DIGEST),
            "a".repeat(64),
        )
        .unwrap();
        let mut delta = tempfile::tempfile().unwrap();
        delta.set_len(page * 2).unwrap();
        let ranges = [VmMemoryRange {
            file_offset: page,
            length: page * 2,
        }];
        assert!(checkpoint_memory(&mut delta, Some(&ranges), Some(&base), &state_dir).is_err());
        assert_eq!(fs::read_dir(&state_dir).unwrap().count(), 0);
    }

    #[test]
    fn delta_publishes_overlay_or_merges_when_overlay_is_larger() {
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        for size in [page, page * 256] {
            let directory = tempfile::tempdir().unwrap();
            let base_dir = directory.path().join("base");
            let state_dir = directory.path().join("checkpoint");
            fs::create_dir(&base_dir).unwrap();
            fs::create_dir(&state_dir).unwrap();
            let base_path = base_dir.join("memory");
            let base = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&base_path)
                .unwrap();
            base.set_len(size).unwrap();
            base.write_all_at(&vec![0xa5; page as usize], 0).unwrap();
            // The base digest is produced when a restored base is verified.
            let digest = copy_flat_sparse_file(
                &mut base.try_clone().unwrap(),
                &directory.path().join("verified-base"),
            )
            .unwrap()
            .sha256;
            fs::write(base_dir.join(VERIFIED_MEMORY_DIGEST), digest).unwrap();
            let mut delta = tempfile::tempfile().unwrap();
            delta.set_len(size).unwrap();
            let ranges = [VmMemoryRange {
                file_offset: 0,
                length: page,
            }];
            let artifacts =
                checkpoint_memory(&mut delta, Some(&ranges), Some(&base), &state_dir).unwrap();
            assert_eq!(fs::read(&base_path).unwrap()[0], 0xa5);
            if size == page {
                assert_eq!(artifacts.len(), 1);
                assert!(!state_dir.join("memory.overlay").exists());
                assert!(fs::read(state_dir.join("memory"))
                    .unwrap()
                    .iter()
                    .all(|byte| *byte == 0));
            } else {
                assert_eq!(artifacts.len(), 2);
                let overlay = fs::read(state_dir.join("memory.overlay")).unwrap();
                assert_eq!(&overlay[..8], b"DBOVLY01");
                assert!(overlay[48..].iter().all(|byte| *byte == 0));
                let restored = directory.path().join("restored");
                restore_overlay(
                    &state_dir.join("memory.overlay"),
                    &restored,
                    SparseMeta {
                        logical_size: size,
                        encoded_size: overlay.len() as u64,
                        sha256: &artifacts[&state_dir.join("memory.overlay")].1.sha256,
                    },
                )
                .unwrap();
                assert_eq!(fs::read(restored).unwrap(), overlay);
            }
        }
    }
    #[test]
    fn delta_checkpoint_supports_cross_filesystem_output() {
        let source_dir = tempfile::tempdir().unwrap();
        let Ok(output_dir) = tempfile::tempdir_in("/dev/shm") else {
            eprintln!("skip cross-filesystem test: /dev/shm is unavailable");
            return;
        };
        if fs::metadata(source_dir.path()).unwrap().dev()
            == fs::metadata(output_dir.path()).unwrap().dev()
        {
            eprintln!("skip cross-filesystem test: temporary directories share a filesystem");
            return;
        }

        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        for size in [page, page * 256] {
            let base_dir = source_dir.path().join(size.to_string());
            let state_dir = output_dir.path().join(size.to_string());
            fs::create_dir(&base_dir).unwrap();
            fs::create_dir(&state_dir).unwrap();
            let base_path = base_dir.join("memory");
            let base = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&base_path)
                .unwrap();
            base.set_len(size).unwrap();
            base.write_all_at(&vec![0xa5; page as usize], 0).unwrap();
            let digest = copy_flat_sparse_file(
                &mut base.try_clone().unwrap(),
                &base_dir.join("verified-base"),
            )
            .unwrap()
            .sha256;
            fs::write(base_dir.join(VERIFIED_MEMORY_DIGEST), &digest).unwrap();
            let mut delta = tempfile::tempfile().unwrap();
            delta.set_len(size).unwrap();
            let ranges = [VmMemoryRange {
                file_offset: 0,
                length: page,
            }];

            let artifacts =
                checkpoint_memory(&mut delta, Some(&ranges), Some(&base), &state_dir).unwrap();
            let memory = state_dir.join("memory");
            verify_flat_sparse_file(&memory, size, &artifacts[&memory].1.sha256).unwrap();
            if size == page {
                assert_eq!(artifacts.len(), 1);
                assert!(fs::read(&memory).unwrap().iter().all(|byte| *byte == 0));
            } else {
                assert_eq!(artifacts.len(), 2);
                assert_eq!(artifacts[&memory].1.sha256, digest);
                assert_ne!(
                    fs::metadata(&memory).unwrap().dev(),
                    base.metadata().unwrap().dev()
                );
                assert_eq!(fs::read(&memory).unwrap(), fs::read(&base_path).unwrap());
            }
        }
    }

    #[test]
    fn dragonball_memory_and_overlay_restore_across_filesystems() {
        use std::os::unix::fs::PermissionsExt;

        let source_dir = tempfile::tempdir().unwrap();
        let Ok(restore_dir) = tempfile::tempdir_in("/dev/shm") else {
            eprintln!("skip cross-filesystem test: /dev/shm is unavailable");
            return;
        };
        if fs::metadata(source_dir.path()).unwrap().dev()
            == fs::metadata(restore_dir.path()).unwrap().dev()
        {
            eprintln!("skip cross-filesystem test: temporary directories share a filesystem");
            return;
        }

        let size = 4 * 1024 * 1024;
        let source = source_dir.path().join("memory");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&source)
            .unwrap();
        file.set_len(size).unwrap();
        file.write_all_at(b"guest memory", size - 12).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();

        let restored = restore_dir.path().join("memory");
        let digest = copy_flat_sparse_file(
            &mut fs::File::open(&source).unwrap(),
            &source_dir.path().join("verified-base"),
        )
        .unwrap()
        .sha256;
        restore_memory(&source, &restored, size, &digest).unwrap();
        verify_flat_sparse_file(&restored, size, &digest).unwrap();
        let metadata = restored.metadata().unwrap();
        assert_eq!(metadata.permissions().mode() & 0o222, 0);
        assert!(metadata.blocks() * 512 < size);

        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let ranges = [VmMemoryRange {
            file_offset: size - page,
            length: page,
        }];
        let overlay_source = source_dir.path().join("memory.overlay");
        let encoded = encode_memory_overlay(&file, &ranges, &overlay_source, &sl!()).unwrap();
        let overlay = restore_dir.path().join("memory.overlay");
        restore_overlay(
            &overlay_source,
            &overlay,
            SparseMeta {
                logical_size: size,
                encoded_size: encoded.encoded_size,
                sha256: &encoded.sha256,
            },
        )
        .unwrap();
        assert_eq!(
            fs::read(&overlay).unwrap(),
            fs::read(&overlay_source).unwrap()
        );
        assert_eq!(overlay.metadata().unwrap().permissions().mode() & 0o222, 0);

        let next = restore_dir.path().join("next");
        fs::create_dir(&next).unwrap();
        let base = fs::File::open(&restored).unwrap();
        let mut memory = fs::File::open(&source).unwrap();
        let artifacts = checkpoint_memory(&mut memory, Some(&ranges), Some(&base), &next).unwrap();
        assert_eq!(artifacts[&next.join("memory")].1.sha256, digest);
        assert_eq!(artifacts.len(), 2);
    }

    #[tokio::test]
    async fn dragonball_flat_sparse_memory_is_linked_for_restore() {
        use std::os::unix::fs::{FileExt, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let checkpoint_root = directory.path().join("checkpoint");
        let state_dir = checkpoint_root.join("vm");
        fs::create_dir_all(&state_dir).unwrap();
        let memory = state_dir.join("memory");
        let memory_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&memory)
            .unwrap();
        memory_file.set_len((2 * 1024 * 1024) as u64).unwrap();
        memory_file
            .write_at(b"guest-memory", 1024 * 1024 + 7)
            .unwrap();
        fs::remove_file(&memory).unwrap();
        fs::write(state_dir.join("state"), b"dragonball-state").unwrap();

        let artifacts = finalize_checkpoint_memory(
            VmSaveResult {
                transient_memory: Some(memory_file),
                ..Default::default()
            },
            &state_dir,
        )
        .unwrap();
        let mut artifacts = artifacts
            .into_iter()
            .map(|(path, value)| VmCheckpointArtifact {
                file: path.file_name().unwrap().to_string_lossy().into_owned(),
                format: value.format,
                logical_size: value.logical_size,
                encoded_size: value.encoded_size,
                sha256: value.sha256,
            })
            .collect::<Vec<_>>();
        fs::set_permissions(state_dir.join("state"), fs::Permissions::from_mode(0o400)).unwrap();
        artifacts.push(VmCheckpointArtifact {
            file: "state".into(),
            format: VM_ARTIFACT_RAW_FORMAT.into(),
            logical_size: 16,
            encoded_size: 16,
            sha256: file_integrity(&state_dir.join("state")).unwrap().1,
        });
        assert!(memory.exists());
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts.iter().any(|artifact| {
            artifact.file == "memory"
                && artifact.format == FLAT_SPARSE_FORMAT
                && artifact.logical_size == 2 * 1024 * 1024
                && artifact.encoded_size == artifact.logical_size
        }));

        let restore_root = directory.path().join("restore");
        fs::create_dir(&restore_root).unwrap();
        let restored = restore_root.join("vm");
        fs::create_dir(&restored).unwrap();
        super::validate_artifacts(
            VmResourceTopology {
                vcpus: 1,
                memory_mb: 2,
            },
            &artifacts,
        )
        .unwrap();
        prepare_restore_memory(state_dir, restored.clone(), artifacts)
            .await
            .unwrap();
        let restored_memory = fs::read(restored.join("memory")).unwrap();
        assert_eq!(restored_memory.len(), 2 * 1024 * 1024);
        assert!(restored_memory[..1024 * 1024].iter().all(|byte| *byte == 0));
        assert_eq!(
            &restored_memory[1024 * 1024 + 7..1024 * 1024 + 19],
            b"guest-memory"
        );
    }
    #[test]
    fn dragonball_manifest_requires_flat_sparse_memory_and_raw_state() {
        let memory = VmCheckpointArtifact {
            file: "memory".to_string(),
            format: FLAT_SPARSE_FORMAT.to_string(),
            logical_size: 128 << 20,
            encoded_size: 128 << 20,
            sha256: "a".repeat(64),
        };
        let state = VmCheckpointArtifact {
            file: "state".to_string(),
            format: VM_ARTIFACT_RAW_FORMAT.to_string(),
            logical_size: 128,
            encoded_size: 128,
            sha256: "b".repeat(64),
        };
        super::validate_artifacts(
            VmResourceTopology {
                vcpus: 1,
                memory_mb: 128,
            },
            &[memory.clone(), state.clone()],
        )
        .unwrap();
        assert!(super::validate_artifacts(
            VmResourceTopology {
                vcpus: 1,
                memory_mb: 64
            },
            &[memory.clone(), state.clone()]
        )
        .is_err());
        assert!(super::validate_artifacts(
            VmResourceTopology {
                vcpus: 1,
                memory_mb: 128
            },
            std::slice::from_ref(&memory)
        )
        .is_err());
        assert!(super::validate_artifacts(
            VmResourceTopology {
                vcpus: 1,
                memory_mb: 128
            },
            &[memory.clone(), state.clone(), state]
        )
        .is_err());
    }
    #[test]
    fn native_codec_errors_keep_artifact_categories() {
        let invalid = map_memory_error(CodecError::Invalid("bad digest".into()).into());
        assert!(matches!(
            invalid.downcast_ref::<ArtifactError>(),
            Some(ArtifactError::Invalid(_))
        ));
        let io =
            map_memory_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into());
        assert!(io.downcast_ref::<std::io::Error>().is_some());
    }
}
