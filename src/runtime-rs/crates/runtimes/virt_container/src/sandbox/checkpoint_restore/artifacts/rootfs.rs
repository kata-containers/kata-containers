// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
use super::super::{
    manifest::{CheckpointRootfs, WORKLOAD_CHECKPOINT_STORAGE},
    sparse::{encode_sparse_artifact, SPARSE_FORMAT},
};
use anyhow::{Context, Result};
use common::{error::Error as CommonError, types::SandboxCheckpointTask};
use hypervisor::{BlockDeviceModern, BlockSourceFormat};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::Instant,
};
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
    _options: &HashMap<String, String>,
    blocks: Vec<BlockDeviceModern>,
) -> Result<Vec<CheckpointRootfs>> {
    let mut blocks_by_id = blocks
        .into_iter()
        .map(|block| (block.device_id.clone(), block))
        .collect::<HashMap<_, _>>();

    let storage_dir = checkpoint_root.join(WORKLOAD_CHECKPOINT_STORAGE);
    fs::create_dir(&storage_dir).context("create checkpoint rootfs storage directory")?;

    let task_count = tasks.len();
    let mut artifacts = Vec::with_capacity(task_count);

    for (position, task) in tasks.iter().enumerate() {
        if task.rootfs_device_ids.len() != 1 {
            return Err(CommonError::SandboxOperationUnsupported(format!(
                "checkpoint task {} requires one writable rootfs",
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
        });
    }

    fs::File::open(&storage_dir)?.sync_all()?;

    Ok(artifacts)
}
#[cfg(test)]
mod tests {
    use super::*;
    use hypervisor::BlockConfigModern;

    #[test]
    fn rootfs_capture_records_task_and_block_identity() {
        let directory = tempfile::tempdir().unwrap();
        let checkpoint_root = directory.path().join("checkpoint");
        fs::create_dir(&checkpoint_root).unwrap();
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

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].checkpoint_key, "app");
        assert_eq!(artifacts[0].device_id, "rootfs-device");
        assert_eq!(artifacts[0].index, 3);
        assert_eq!(artifacts[0].logical_size, contents.len() as u64);
        assert_eq!(
            fs::metadata(checkpoint_root.join(&artifacts[0].file))
                .unwrap()
                .len(),
            artifacts[0].encoded_size
        );
    }

    #[test]
    fn managed_rootfs_flushes_a_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let rootfs = directory.path().join("rootfs.raw");
        fs::write(&rootfs, b"rootfs contents").unwrap();

        flush_managed_rootfs(&rootfs).unwrap();
    }
}
