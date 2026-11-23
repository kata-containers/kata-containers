// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::super::{
    manifest::{CheckpointRootfs, CheckpointTask, ImmutableRootfs, WORKLOAD_CHECKPOINT_STORAGE},
    sparse::{decode_sparse_artifact, encode_sparse_artifact, SparseMeta, SPARSE_FORMAT},
};
use super::{run_artifact_worker, IMMUTABLE_ROOTFS_OPTION};
use anyhow::{Context, Result};
use common::{error::Error as CommonError, types::SandboxCheckpointTask};
use hypervisor::{BlockConfigModern, BlockDeviceAio, BlockDeviceModern, BlockSourceFormat};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::Instant,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImmutableRootfsOption {
    checkpoint_key: String,
    #[serde(default)]
    path: String,
    size: u64,
    sha256: String,
}

fn immutable_rootfs_options(
    options: &HashMap<String, String>,
) -> Result<HashMap<String, ImmutableRootfsOption>> {
    let Some(value) = options.get(IMMUTABLE_ROOTFS_OPTION) else {
        return Ok(HashMap::new());
    };

    let entries: Vec<ImmutableRootfsOption> = serde_json::from_str(value).map_err(|err| {
        CommonError::InvalidSandboxOperation(format!("decode immutable rootfs inventory: {err}"))
    })?;
    let mut result = HashMap::with_capacity(entries.len());
    for entry in entries {
        if entry.checkpoint_key.is_empty()
            || entry.size == 0
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || result.insert(entry.checkpoint_key.clone(), entry).is_some()
        {
            return Err(CommonError::InvalidSandboxOperation(
                "invalid or duplicate immutable rootfs inventory".to_string(),
            )
            .into());
        }
    }

    Ok(result)
}

fn rootfs_artifact_name(task_count: usize, position: usize) -> String {
    if task_count == 1 {
        "rootfs-base.sparse".to_string()
    } else {
        format!("rootfs-{position}-base.sparse")
    }
}

fn flush_managed_rootfs(source: &Path) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)
        .context(format!(
            "open managed rootfs block file {} for flush",
            source.display()
        ))?
        .sync_all()
        .with_context(|| format!("flush managed rootfs block file {}", source.display()))?;

    Ok(())
}

pub(in crate::sandbox::checkpoint_restore) fn write_rootfs_artifacts(
    checkpoint_root: &Path,
    tasks: &[SandboxCheckpointTask],
    options: &HashMap<String, String>,
    blocks: Vec<BlockDeviceModern>,
) -> Result<Vec<CheckpointRootfs>> {
    let mut blocks_by_id = blocks
        .into_iter()
        .map(|block| (block.device_id.clone(), block))
        .collect::<HashMap<_, _>>();

    let storage_dir = checkpoint_root.join(WORKLOAD_CHECKPOINT_STORAGE);
    fs::create_dir(&storage_dir).context("create checkpoint rootfs storage directory")?;

    let mut immutable_options = immutable_rootfs_options(options)?;
    let task_count = tasks.len();
    let mut artifacts = Vec::with_capacity(task_count);

    for (position, task) in tasks.iter().enumerate() {
        if task.rootfs_device_ids.is_empty() || task.rootfs_device_ids.len() > 2 {
            return Err(CommonError::SandboxOperationUnsupported(format!(
                "checkpoint task {} requires one writable rootfs and at most one immutable lower",
                task.task_id
            ))
            .into());
        }

        let mut task_blocks = Vec::with_capacity(task.rootfs_device_ids.len());
        for device_id in &task.rootfs_device_ids {
            let Some(block) = blocks_by_id.remove(device_id) else {
                return Err(CommonError::SandboxOperationPrecondition(format!(
                    "checkpoint task {} rootfs device {} is absent",
                    task.task_id, device_id
                ))
                .into());
            };
            task_blocks.push(block);
        }

        let writable_position = task_blocks
            .iter()
            .position(|block| !block.config.is_readonly)
            .ok_or_else(|| {
                CommonError::SandboxOperationPrecondition(format!(
                    "checkpoint task {} has no writable rootfs device",
                    task.task_id
                ))
            })?;

        if task_blocks
            .iter()
            .filter(|block| !block.config.is_readonly)
            .count()
            != 1
        {
            return Err(CommonError::SandboxOperationUnsupported(format!(
                "checkpoint task {} has multiple writable rootfs devices",
                task.task_id
            ))
            .into());
        }

        let block = task_blocks.remove(writable_position);
        let immutable_lower = match task_blocks.pop() {
            None => {
                if immutable_options.remove(&task.checkpoint_key).is_some() {
                    return Err(CommonError::InvalidSandboxOperation(format!(
                        "task {} supplied immutable rootfs identity without a readonly device",
                        task.task_id
                    ))
                    .into());
                }
                None
            }
            Some(lower) if lower.config.is_readonly => {
                let identity = immutable_options
                    .remove(&task.checkpoint_key)
                    .ok_or_else(|| {
                        CommonError::InvalidSandboxOperation(format!(
                            "task {} is missing immutable rootfs identity",
                            task.task_id
                        ))
                    })?;

                let source = Path::new(&lower.config.path_on_host);
                let metadata = fs::symlink_metadata(source)
                    .context(format!("inspect immutable rootfs {}", source.display()))?;
                if !source.is_absolute()
                    || metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() != identity.size
                    || metadata.permissions().mode() & 0o222 != 0
                    || !identity.path.is_empty()
                {
                    return Err(CommonError::InvalidSandboxOperation(format!(
                        "task {} immutable rootfs identity is invalid",
                        task.task_id
                    ))
                    .into());
                }

                Some(ImmutableRootfs {
                    device_id: lower.device_id,
                    index: lower.config.index,
                    driver_option: lower.config.driver_option,
                    virt_path: lower.config.virt_path,
                    is_direct: lower.config.is_direct,
                    discard_unmap: lower.config.discard_unmap,
                    aio: lower.config.blkdev_aio.to_string(),
                    queue_size: lower.config.queue_size,
                    num_queues: lower.config.num_queues,
                    logical_sector_size: lower.config.logical_sector_size,
                    physical_sector_size: lower.config.physical_sector_size,
                    serial_override: lower.config.serial_override,
                    size: identity.size,
                    sha256: identity.sha256.to_lowercase(),
                })
            }
            Some(_) => {
                return Err(CommonError::SandboxOperationUnsupported(format!(
                    "checkpoint task {} has multiple writable rootfs devices",
                    task.task_id
                ))
                .into())
            }
        };

        let source = Path::new(&block.config.path_on_host);
        let metadata = fs::symlink_metadata(source)?;

        if !source.is_absolute()
            || metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !matches!(block.config.source, BlockSourceFormat::Raw)
        {
            return Err(CommonError::SandboxOperationPrecondition(format!(
                "managed rootfs block file {} is not a regular raw file",
                source.display()
            ))
            .into());
        }

        flush_managed_rootfs(source)?;

        let relative = format!(
            "{WORKLOAD_CHECKPOINT_STORAGE}/{}",
            rootfs_artifact_name(task_count, position)
        );
        let destination = checkpoint_root.join(&relative);
        let partial = destination.with_extension("sparse.partial");

        let encode_started = Instant::now();
        let encoded = match encode_sparse_artifact(source, &partial) {
            Ok(encoded) => encoded,
            Err(err) => {
                let _ = fs::remove_file(&partial);
                return Err(err);
            }
        };

        fs::rename(&partial, &destination)?;
        debug!(
            sl!(),
            "encoded canonical sparse rootfs artifact";
            "logical_bytes" => encoded.logical_size,
            "encoded_bytes" => encoded.encoded_size,
            "non_zero_chunks" => encoded.non_zero_chunks,
            "candidate_chunks" => encoded.stats.candidate_chunks,
            "candidate_bytes" => encoded.stats.candidate_bytes,
            "written_bytes" => encoded.stats.written_bytes,
            "elapsed_us" => encode_started.elapsed().as_micros(),
        );

        artifacts.push(CheckpointRootfs {
            checkpoint_key: task.checkpoint_key.clone(),
            file: relative,
            format: SPARSE_FORMAT.to_string(),
            device_id: block.device_id.clone(),
            index: block.config.index,
            driver_option: block.config.driver_option.clone(),
            virt_path: block.config.virt_path.clone(),
            is_direct: block.config.is_direct,
            discard_unmap: block.config.discard_unmap,
            aio: block.config.blkdev_aio.to_string(),
            queue_size: block.config.queue_size,
            num_queues: block.config.num_queues,
            logical_sector_size: block.config.logical_sector_size,
            physical_sector_size: block.config.physical_sector_size,
            serial_override: block.config.serial_override.clone(),
            logical_size: encoded.logical_size,
            encoded_size: encoded.encoded_size,
            sha256: encoded.sha256,
            immutable_lower,
        });
    }

    if !immutable_options.is_empty() {
        return Err(CommonError::InvalidSandboxOperation(
            "immutable rootfs inventory contains unknown tasks".to_string(),
        )
        .into());
    }

    fs::File::open(&storage_dir)?.sync_all()?;

    Ok(artifacts)
}

pub(in crate::sandbox::checkpoint_restore) async fn restore_rootfs_artifacts(
    checkpoint_root: &Path,
    tasks: &[CheckpointTask],
    options: &HashMap<String, String>,
    rootfs: &[CheckpointRootfs],
    restore_dir: &Path,
    sync_private_copy: bool,
) -> Result<Vec<BlockDeviceModern>> {
    if rootfs.len() != tasks.len() {
        return Err(CommonError::InvalidSandboxOperation(
            "restore rootfs inventory does not match tasks".to_string(),
        )
        .into());
    }

    let tasks = tasks
        .iter()
        .map(|task| (task.checkpoint_key.as_str(), task))
        .collect::<HashMap<_, _>>();

    let mut immutable_options = immutable_rootfs_options(options)?;
    let mut blocks = Vec::with_capacity(rootfs.len() * 2);
    let task_count = rootfs.len();

    for (position, artifact) in rootfs.iter().enumerate() {
        let task = tasks.get(artifact.checkpoint_key.as_str()).ok_or_else(|| {
            CommonError::InvalidSandboxOperation(format!(
                "rootfs checkpoint key {} has no restored task",
                artifact.checkpoint_key
            ))
        })?;
        let expected = format!(
            "{WORKLOAD_CHECKPOINT_STORAGE}/{}",
            rootfs_artifact_name(task_count, position)
        );

        if artifact.file != expected
            || artifact.format != SPARSE_FORMAT
            || task.checkpoint_key.is_empty()
        {
            return Err(CommonError::InvalidSandboxOperation(
                "invalid rootfs artifact path or task key".to_string(),
            )
            .into());
        }

        let source = checkpoint_root.join(&artifact.file);
        let metadata = fs::symlink_metadata(&source)?;

        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.permissions().mode() & 0o222 != 0
        {
            return Err(CommonError::InvalidSandboxOperation(format!(
                "checkpoint rootfs artifact {} is invalid",
                source.display()
            ))
            .into());
        }

        let private = if task_count == 1 {
            restore_dir.join("rootfs.raw")
        } else {
            restore_dir.join(format!("rootfs-{position}.raw"))
        };

        let source_clone = source.clone();
        let private_clone = private.clone();
        let expected_sha256 = artifact.sha256.clone();
        let encoded_size = artifact.encoded_size;
        let logical_size = artifact.logical_size;

        run_artifact_worker(move || {
            let decode_started = Instant::now();
            let decoded = decode_sparse_artifact(
                &source_clone,
                &private_clone,
                SparseMeta {
                    encoded_size,
                    logical_size,
                    sha256: &expected_sha256,
                },
                0o600,
                sync_private_copy,
            )?;

            debug!(
                sl!(),
                "decoded canonical sparse rootfs artifact";
                "logical_bytes" => logical_size,
                "encoded_bytes" => encoded_size,
                "records" => decoded.records,
                "written_bytes" => decoded.written_bytes,
                "elapsed_us" => decode_started.elapsed().as_micros(),
            );

            Ok(())
        })
        .await?;

        let block = BlockDeviceModern {
            device_id: artifact.device_id.clone(),
            config: BlockConfigModern {
                path_on_host: private.display().to_string(),
                is_readonly: false,
                discard_unmap: artifact.discard_unmap,
                source: BlockSourceFormat::Raw,
                is_direct: artifact.is_direct,
                index: artifact.index,
                blkdev_aio: BlockDeviceAio::new(&artifact.aio),
                driver_option: artifact.driver_option.clone(),
                virt_path: artifact.virt_path.clone(),
                queue_size: artifact.queue_size,
                num_queues: artifact.num_queues,
                logical_sector_size: artifact.logical_sector_size,
                physical_sector_size: artifact.physical_sector_size,
                serial_override: artifact.serial_override.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        blocks.push(block);

        if let Some(lower) = &artifact.immutable_lower {
            let identity = immutable_options
                .remove(&artifact.checkpoint_key)
                .ok_or_else(|| {
                    CommonError::InvalidSandboxOperation(format!(
                        "rootfs {} is missing immutable lower resolution",
                        artifact.checkpoint_key
                    ))
                })?;

            let source = Path::new(&identity.path);
            let metadata = fs::symlink_metadata(source)?;

            if !source.is_absolute()
                || metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.permissions().mode() & 0o222 != 0
                || metadata.len() != lower.size
                || identity.size != lower.size
                || identity.sha256.to_lowercase() != lower.sha256
            {
                return Err(CommonError::InvalidSandboxOperation(format!(
                    "rootfs {} immutable lower does not match checkpoint",
                    artifact.checkpoint_key
                ))
                .into());
            }

            blocks.push(BlockDeviceModern {
                device_id: lower.device_id.clone(),
                config: BlockConfigModern {
                    path_on_host: identity.path,
                    is_readonly: true,
                    discard_unmap: lower.discard_unmap,
                    source: BlockSourceFormat::Raw,
                    is_direct: lower.is_direct,
                    index: lower.index,
                    blkdev_aio: BlockDeviceAio::new(&lower.aio),
                    driver_option: lower.driver_option.clone(),
                    virt_path: lower.virt_path.clone(),
                    queue_size: lower.queue_size,
                    num_queues: lower.num_queues,
                    logical_sector_size: lower.logical_sector_size,
                    physical_sector_size: lower.physical_sector_size,
                    serial_override: lower.serial_override.clone(),
                    ..Default::default()
                },
                ..Default::default()
            });
        }
    }

    if !immutable_options.is_empty() {
        return Err(CommonError::InvalidSandboxOperation(
            "immutable rootfs resolution contains unknown tasks".to_string(),
        )
        .into());
    }

    fs::File::open(restore_dir)?.sync_all()?;

    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rootfs_round_trip_uses_only_artifact_inputs() {
        let directory = tempfile::tempdir().unwrap();
        let checkpoint_root = directory.path().join("checkpoint");
        let restore_root = directory.path().join("restore");
        fs::create_dir(&checkpoint_root).unwrap();
        fs::create_dir(&restore_root).unwrap();
        let source = directory.path().join("writable.raw");
        let mut contents = vec![0; 128 * 1024];
        contents[65536..65548].copy_from_slice(b"rootfs-state");
        fs::write(&source, &contents).unwrap();
        let tasks = vec![SandboxCheckpointTask {
            checkpoint_key: "app".into(),
            task_id: "old-app".into(),
            rootfs_device_ids: vec!["rootfs-device".into()],
        }];
        let block = BlockDeviceModern {
            device_id: "rootfs-device".into(),
            config: BlockConfigModern {
                path_on_host: source.display().to_string(),
                source: BlockSourceFormat::Raw,
                index: 3,
                driver_option: "virtio-blk".into(),
                virt_path: "/dev/vdd".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let options = HashMap::new();
        let artifacts =
            write_rootfs_artifacts(&checkpoint_root, &tasks, &options, vec![block]).unwrap();

        let error = restore_rootfs_artifacts(
            &checkpoint_root,
            &[],
            &options,
            &artifacts,
            &restore_root,
            true,
        )
        .await
        .expect_err("task inventory mismatch must fail");
        assert!(matches!(
            error.downcast_ref::<CommonError>(),
            Some(CommonError::InvalidSandboxOperation(message))
                if message == "restore rootfs inventory does not match tasks"
        ));
        assert!(!restore_root.join("rootfs.raw").exists());

        let restored = restore_rootfs_artifacts(
            &checkpoint_root,
            &[CheckpointTask {
                checkpoint_key: "app".into(),
                task_id: "old-app".into(),
            }],
            &options,
            &artifacts,
            &restore_root,
            true,
        )
        .await
        .unwrap();
        assert_eq!(restored.len(), 1);
        let block = &restored[0];
        assert_eq!(block.device_id, "rootfs-device");
        assert_eq!(block.config.index, 3);
        assert_eq!(block.config.driver_option, "virtio-blk");
        assert_eq!(block.config.virt_path, "/dev/vdd");
        assert!(!block.config.is_readonly);
        assert_eq!(fs::read(&block.config.path_on_host).unwrap(), contents);
    }

    #[test]
    fn immutable_rootfs_inventory_is_strict_and_keyed() {
        let mut options = HashMap::new();
        options.insert(
            IMMUTABLE_ROOTFS_OPTION.to_string(),
            format!(
                r#"[{{"checkpoint_key":"app","size":4096,"sha256":"{}"}}]"#,
                "a".repeat(64)
            ),
        );
        let entries = immutable_rootfs_options(&options).unwrap();
        let entry = entries.get("app").unwrap();
        assert_eq!(entry.size, 4096);
        assert!(entry.path.is_empty());

        options.insert(
            IMMUTABLE_ROOTFS_OPTION.to_string(),
            r#"[{"checkpoint_key":"app","size":4096,"sha256":"bad"}]"#.to_string(),
        );
        assert!(immutable_rootfs_options(&options).is_err());
    }

    #[test]
    fn managed_rootfs_flushes_a_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let rootfs = directory.path().join("rootfs.raw");
        fs::write(&rootfs, b"rootfs contents").unwrap();

        flush_managed_rootfs(&rootfs).unwrap();
    }
}
