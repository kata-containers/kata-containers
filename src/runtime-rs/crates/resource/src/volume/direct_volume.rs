// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{get_mount_path, get_mount_type};
use kata_types::{mount::DirectVolumeMountInfo, qcow2::parse_qcow2_volume};
use nix::sys::{stat, stat::SFlag};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::{
    direct_volumes::{
        get_direct_volume_path, rawblock_volume, spdk_volume, vfio_volume, volume_mount_info,
        KATA_DIRECT_VOLUME_TYPE, KATA_SPDK_VOLUME_TYPE, KATA_SPOOL_VOLUME_TYPE,
        KATA_VFIO_VOLUME_TYPE,
    },
    utils::KATA_MOUNT_BIND_TYPE,
    Volume,
};

enum DirectVolumeType {
    RawBlock,
    Spdk,
    Vfio,
}

fn to_volume_type(volume_type: &str) -> DirectVolumeType {
    match volume_type {
        KATA_SPDK_VOLUME_TYPE | KATA_SPOOL_VOLUME_TYPE => DirectVolumeType::Spdk,
        KATA_VFIO_VOLUME_TYPE => DirectVolumeType::Vfio,
        _ => DirectVolumeType::RawBlock,
    }
}

/// SPDK and VFIO direct volumes hand the guest whatever their backend holds.
/// Reject qcow2 metadata on them rather than ignore it, so a qcow2 image is
/// never attached to the guest as a raw disk.
fn reject_qcow2_metadata(mount_info: &DirectVolumeMountInfo) -> Result<()> {
    if parse_qcow2_volume(&mount_info.metadata)?.is_some() {
        return Err(anyhow!(
            "{} direct volumes do not support qcow2",
            mount_info.volume_type
        ));
    }
    Ok(())
}

pub(crate) async fn handle_direct_volume(
    d: &RwLock<DeviceManager>,
    m: &oci::Mount,
    read_only: bool,
    sid: &str,
    discard_supported: bool,
) -> Result<Option<Arc<dyn Volume>>> {
    // In the direct volume scenario, we check if the source of a mount is in the
    // /run/kata-containers/shared/direct-volumes/SID path by iterating over all the mounts.
    // If the source is not in the path with error kind *NotFound*, we ignore the error
    // and we treat it as block volume with oci Mount.type *bind*. Just fill in the block
    // volume info in the DirectVolumeMountInfo
    let mount_info: DirectVolumeMountInfo = match volume_mount_info(&get_mount_path(m.source())) {
        Ok(mount_info) => mount_info,
        Err(e) => {
            // First, We need filter the non-io::ErrorKind.
            if !e.is::<std::io::ErrorKind>() {
                return Err(anyhow!(format!(
                    "unexpected error occurs when parse mount info for {:?}, with error {:?}",
                    m.source(),
                    e.to_string()
                )));
            }

            // Second, we need filter non-NotFound error.
            // Safe to unwrap here, as the error is of type std::io::ErrorKind.
            let error_kind = e.downcast_ref::<std::io::ErrorKind>().unwrap();
            if *error_kind != std::io::ErrorKind::NotFound {
                return Err(anyhow!(format!(
                    "failed to parse volume mount info for {:?}, with error {:?}",
                    m.source(),
                    e.to_string()
                )));
            }

            // Third, if the case is *NotFound* , we just return Ok(None).
            return Ok(None);
        }
    };

    let volume_type = to_volume_type(mount_info.volume_type.as_str());
    if !matches!(volume_type, DirectVolumeType::RawBlock) {
        reject_qcow2_metadata(&mount_info)
            .with_context(|| format!("invalid direct volume {m:?}"))?;
    }

    let direct_volume: Arc<dyn Volume> = match volume_type {
        DirectVolumeType::RawBlock => Arc::new(
            rawblock_volume::RawblockVolume::new(
                d,
                m,
                &mount_info,
                read_only,
                sid,
                discard_supported,
            )
            .await
            .with_context(|| format!("new sid {:?} rawblock volume {:?}", sid, m))?,
        ),
        DirectVolumeType::Spdk => Arc::new(
            spdk_volume::SPDKVolume::new(d, m, &mount_info, read_only, sid)
                .await
                .with_context(|| format!("create spdk volume {m:?}"))?,
        ),
        DirectVolumeType::Vfio => Arc::new(
            vfio_volume::VfioVolume::new(d, m, &mount_info, read_only, sid)
                .await
                .with_context(|| format!("new vfio volume {m:?}"))?,
        ),
    };

    Ok(Some(direct_volume))
}

pub(crate) fn is_direct_volume(m: &oci::Mount) -> Result<bool> {
    let mnt_type = get_mount_type(m);
    let mount_type = mnt_type.as_str();

    // Filter the non-bind volume and non-direct-vol volume
    let vol_types = [
        KATA_MOUNT_BIND_TYPE,
        KATA_DIRECT_VOLUME_TYPE,
        KATA_VFIO_VOLUME_TYPE,
        KATA_SPDK_VOLUME_TYPE,
        KATA_SPOOL_VOLUME_TYPE,
    ];
    if !vol_types.contains(&mount_type) {
        return Ok(false);
    }

    match get_direct_volume_path(get_mount_path(m.source()).as_str()) {
        Ok(directvol_path) => {
            let fstat = stat::stat(directvol_path.as_str())
                .context(format!("stat mount source {directvol_path} failed."))?;
            Ok(SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFDIR)
        }
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::qcow2::{DIRECT_VOLUME_DISCARD_KEY, DIRECT_VOLUME_FORMAT_KEY};

    #[test]
    fn spdk_and_vfio_volumes_reject_qcow2_metadata() {
        struct TestData<'a> {
            volume_type: &'a str,
            metadata: &'a [(&'a str, &'a str)],
            // Empty for success, otherwise text the error must contain.
            err: &'a str,
        }
        let tests = [
            TestData {
                volume_type: KATA_SPDK_VOLUME_TYPE,
                metadata: &[],
                err: "",
            },
            TestData {
                volume_type: KATA_SPDK_VOLUME_TYPE,
                metadata: &[("num_queues", "4"), (DIRECT_VOLUME_FORMAT_KEY, "raw")],
                err: "",
            },
            TestData {
                volume_type: KATA_SPDK_VOLUME_TYPE,
                metadata: &[(DIRECT_VOLUME_FORMAT_KEY, "qcow2")],
                err: "spdkvol direct volumes do not support qcow2",
            },
            TestData {
                volume_type: KATA_SPOOL_VOLUME_TYPE,
                metadata: &[(DIRECT_VOLUME_FORMAT_KEY, "qcow2")],
                err: "spoolvol direct volumes do not support qcow2",
            },
            TestData {
                volume_type: KATA_VFIO_VOLUME_TYPE,
                metadata: &[(DIRECT_VOLUME_FORMAT_KEY, "qcow2")],
                err: "vfiovol direct volumes do not support qcow2",
            },
            TestData {
                volume_type: KATA_VFIO_VOLUME_TYPE,
                metadata: &[(DIRECT_VOLUME_DISCARD_KEY, "unmap")],
                err: r#"requires "io.katacontainers.volume.format" to be "qcow2""#,
            },
            TestData {
                volume_type: KATA_SPDK_VOLUME_TYPE,
                metadata: &[(DIRECT_VOLUME_FORMAT_KEY, "vhd")],
                err: r#"unsupported volume format "vhd""#,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let mount_info = DirectVolumeMountInfo {
                volume_type: d.volume_type.to_string(),
                device: String::from("/dev/sdx"),
                fs_type: String::from("ext4"),
                metadata: d
                    .metadata
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                options: Vec::new(),
            };
            let result = reject_qcow2_metadata(&mount_info);
            let msg = format!(
                "test[{}] {} {:?}: {:?}",
                i, d.volume_type, d.metadata, result
            );
            if d.err.is_empty() {
                assert!(result.is_ok(), "{}", msg);
            } else {
                let actual = format!("{:#}", result.unwrap_err());
                assert!(actual.contains(d.err), "{}", msg);
            }
        }
    }
}
