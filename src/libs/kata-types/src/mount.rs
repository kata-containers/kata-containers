// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Error, Result};
use std::convert::TryFrom;
use std::{collections::HashMap, fs, path::PathBuf};

use crate::handler::HandlerManager;

/// Prefix to mark a volume as Kata special.
pub const KATA_VOLUME_TYPE_PREFIX: &str = "kata:";

/// The Mount should be ignored by the host and handled by the guest.
pub const KATA_GUEST_MOUNT_PREFIX: &str = "kata:guest-mount:";

/// The sharedfs volume is mounted by guest OS before starting the kata-agent.
pub const KATA_SHAREDFS_GUEST_PREMOUNT_TAG: &str = "kataShared";

/// KATA_EPHEMERAL_DEV_TYPE creates a tmpfs backed volume for sharing files between containers.
pub const KATA_EPHEMERAL_VOLUME_TYPE: &str = "ephemeral";

/// KATA_HOST_DIR_TYPE use for host empty dir
pub const KATA_HOST_DIR_VOLUME_TYPE: &str = "kata:hostdir";

/// KATA_MOUNT_INFO_FILE_NAME is used for the file that holds direct-volume mount info
pub const KATA_MOUNT_INFO_FILE_NAME: &str = "mountInfo.json";

/// Specify `fsgid` for a volume or mount, `fsgid=1`.
pub const KATA_MOUNT_OPTION_FS_GID: &str = "fsgid";

/// KATA_DIRECT_VOLUME_ROOT_PATH is the root path used for concatenating with the direct-volume mount info file path
pub const KATA_DIRECT_VOLUME_ROOT_PATH: &str = "/run/kata-containers/shared/direct-volumes";

/// SANDBOX_BIND_MOUNTS_DIR is for sandbox bindmounts
pub const SANDBOX_BIND_MOUNTS_DIR: &str = "sandbox-mounts";

/// SANDBOX_BIND_MOUNTS_RO is for sandbox bindmounts with readonly
pub const SANDBOX_BIND_MOUNTS_RO: &str = ":ro";

/// SANDBOX_BIND_MOUNTS_RO is for sandbox bindmounts with readwrite
pub const SANDBOX_BIND_MOUNTS_RW: &str = ":rw";

/// Directly assign a block volume to vm and mount it inside guest.
pub const KATA_VIRTUAL_VOLUME_DIRECT_BLOCK: &str = "direct_block";
/// Present a container image as a generic block device.
pub const KATA_VIRTUAL_VOLUME_IMAGE_RAW_BLOCK: &str = "image_raw_block";
/// Present each container image layer as a generic block device.
pub const KATA_VIRTUAL_VOLUME_LAYER_RAW_BLOCK: &str = "layer_raw_block";
/// Present a container image as a nydus block device.
pub const KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_BLOCK: &str = "image_nydus_block";
/// Present each container image layer as a nydus block device.
pub const KATA_VIRTUAL_VOLUME_LAYER_NYDUS_BLOCK: &str = "layer_nydus_block";
/// Present a container image as a nydus filesystem.
pub const KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_FS: &str = "image_nydus_fs";
/// Present each container image layer as a nydus filesystem.
pub const KATA_VIRTUAL_VOLUME_LAYER_NYDUS_FS: &str = "layer_nydus_fs";
/// Download and extra container image inside guest vm.
pub const KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL: &str = "image_guest_pull";

/// Manager to manage registered storage device handlers.
pub type StorageHandlerManager<H> = HandlerManager<H>;

/// Information about a mount.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Mount {
    /// A device name, but can also be a file or directory name for bind mounts or a dummy.
    /// Path values for bind mounts are either absolute or relative to the bundle. A mount is a
    /// bind mount if it has either bind or rbind in the options.
    pub source: String,
    /// Destination of mount point: path inside container. This value MUST be an absolute path.
    pub destination: PathBuf,
    /// The type of filesystem for the mountpoint.
    pub fs_type: String,
    /// Mount options for the mountpoint.
    pub options: Vec<String>,
    /// Optional device id for the block device when:
    /// - the source is a block device or a mountpoint for a block device
    /// - block device direct assignment is enabled
    pub device_id: Option<String>,
    /// Intermediate path to mount the source on host side and then passthrough to vm by shared fs.
    pub host_shared_fs_path: Option<PathBuf>,
    /// Whether to mount the mountpoint in readonly mode
    pub read_only: bool,
}

impl Mount {
    /// Get size of mount options.
    pub fn option_size(&self) -> usize {
        self.options.iter().map(|v| v.len() + 1).sum()
    }
}

/// DirectVolumeMountInfo contains the information needed by Kata
/// to consume a host block device and mount it as a filesystem inside the guest VM.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct DirectVolumeMountInfo {
    /// The type of the volume (ie. block)
    #[serde(rename = "volume-type")]
    pub volume_type: String,
    /// The device backing the volume.
    pub device: String,
    /// The filesystem type to be mounted on the volume.
    #[serde(rename = "fstype")]
    pub fs_type: String,
    /// Additional metadata to pass to the agent regarding this volume.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    /// Additional mount options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

/// Nydus extra options
#[derive(Debug, serde::Deserialize)]
pub struct NydusExtraOptions {
    /// source path
    pub source: String,
    /// nydus config
    pub config: String,
    /// snapshotter directory
    #[serde(rename(deserialize = "snapshotdir"))]
    pub snapshot_dir: String,
    /// fs version
    pub fs_version: String,
}

impl NydusExtraOptions {
    /// Create Nydus extra options
    pub fn new(mount: &Mount) -> Result<Self> {
        let options: Vec<&str> = mount
            .options
            .iter()
            .filter(|x| x.starts_with("extraoption="))
            .map(|x| x.as_ref())
            .collect();

        if options.len() != 1 {
            return Err(anyhow!(
                "get_nydus_extra_options: Invalid nydus options: {:?}",
                &mount.options
            ));
        }
        let config_raw_data = options[0].trim_start_matches("extraoption=");
        let extra_options_buf =
            base64::decode(config_raw_data).context("decode the nydus's base64 extraoption")?;

        serde_json::from_slice(&extra_options_buf).context("deserialize nydus's extraoption")
    }
}

/// Configuration information for DmVerity device.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct DmVerityInfo {
    /// Hash algorithm for dm-verity.
    pub hashtype: String,
    /// Root hash for device verification or activation.
    pub hash: String,
    /// Size of data device used in verification.
    pub blocknum: u64,
    /// Used block size for the data device.
    pub blocksize: u64,
    /// Used block size for the hash device.
    pub hashsize: u64,
    /// Offset of hash area/superblock on hash_device.
    pub offset: u64,
}

/// Information about directly assigned volume.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct DirectAssignedVolume {
    /// Meta information for directly assigned volume.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Information about pulling image inside guest.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct ImagePullVolume {
    /// Meta information for pulling image inside guest.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Information about nydus image volume.
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct NydusImageVolume {
    /// Nydus configuration information.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config: String,

    /// Nydus snapshot directory
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub snapshot_dir: String,
}

/// Kata virtual volume to encapsulate information for extra mount options and direct volumes.
///
/// It's very expensive to build direct communication channels to pass information:
/// - between snapshotters and kata-runtime/kata-agent/image-rs
/// - between CSI drivers and kata-runtime/kata-agent
///
/// So `KataVirtualVolume` is introduced to encapsulate extra mount options and direct volume
/// information, so we can build a common infrastructure to handle them.
/// `KataVirtualVolume` is a superset of `NydusExtraOptions` and `DirectVolumeMountInfo`.
///
/// Value of `volume_type` determines how to interpret other fields in the structure.
///
/// - `KATA_VIRTUAL_VOLUME_IGNORE`
/// -- all other fields should be ignored/unused.
///
/// - `KATA_VIRTUAL_VOLUME_DIRECT_BLOCK`
/// -- `source`: the directly assigned block device
/// -- `fs_type`: filesystem type
/// -- `options`: mount options
/// -- `direct_volume`: additional metadata to pass to the agent regarding this volume.
///
/// - `KATA_VIRTUAL_VOLUME_IMAGE_RAW_BLOCK` or `KATA_VIRTUAL_VOLUME_LAYER_RAW_BLOCK`
/// -- `source`: path to the raw block image for the container image or layer.
/// -- `fs_type`: filesystem type
/// -- `options`: mount options
/// -- `dm_verity`: disk dm-verity information
///
/// - `KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_BLOCK` or `KATA_VIRTUAL_VOLUME_LAYER_NYDUS_BLOCK`
/// -- `source`: path to nydus meta blob
/// -- `fs_type`: filesystem type
/// -- `nydus_image`: configuration information for nydus image.
/// -- `dm_verity`: disk dm-verity information
///
/// - `KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_FS` or `KATA_VIRTUAL_VOLUME_LAYER_NYDUS_FS`
/// -- `source`: path to nydus meta blob
/// -- `fs_type`: filesystem type
/// -- `nydus_image`: configuration information for nydus image.
///
/// - `KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL`
/// -- `source`: image reference
/// -- `image_pull`: metadata for image pulling
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct KataVirtualVolume {
    /// Type of virtual volume.
    pub volume_type: String,
    /// Source/device path for the virtual volume.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// Filesystem type.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fs_type: String,
    /// Mount options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,

    /// Information about directly assigned volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_volume: Option<DirectAssignedVolume>,
    /// Information about pulling image inside guest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_pull: Option<ImagePullVolume>,
    /// Information about nydus image volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nydus_image: Option<NydusImageVolume>,
    /// DmVerity: configuration information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_verity: Option<DmVerityInfo>,
}

impl KataVirtualVolume {
    /// Create a new instance of `KataVirtualVolume` with specified type.
    pub fn new(volume_type: String) -> Self {
        Self {
            volume_type,
            ..Default::default()
        }
    }

    /// Validate virtual volume object.
    pub fn validate(&self) -> Result<()> {
        match self.volume_type.as_str() {
            KATA_VIRTUAL_VOLUME_DIRECT_BLOCK => {
                if self.source.is_empty() {
                    return Err(anyhow!(
                        "missing source device for directly assigned block volume"
                    ));
                } else if self.fs_type.is_empty() {
                    return Err(anyhow!(
                        "missing filesystem for directly assigned block volume"
                    ));
                }
            }
            KATA_VIRTUAL_VOLUME_IMAGE_RAW_BLOCK | KATA_VIRTUAL_VOLUME_LAYER_RAW_BLOCK => {
                if self.source.is_empty() {
                    return Err(anyhow!("missing source device for raw block volume"));
                } else if self.fs_type.is_empty() {
                    return Err(anyhow!("missing filesystem for raw block volume"));
                }
            }
            KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_BLOCK | KATA_VIRTUAL_VOLUME_LAYER_NYDUS_BLOCK => {
                if self.source.is_empty() {
                    return Err(anyhow!("missing meta blob for nydus block volume"));
                } else if self.fs_type.as_str() != "rafsv6" {
                    return Err(anyhow!("invalid filesystem for nydus block volume"));
                }
                match self.nydus_image.as_ref() {
                    None => {
                        return Err(anyhow!(
                            "missing nydus configuration info for nydus block volume"
                        ))
                    }
                    Some(nydus) => {
                        if nydus.config.is_empty() {
                            return Err(anyhow!(
                                "missing configuration info for nydus block volume"
                            ));
                        } else if nydus.snapshot_dir.is_empty() {
                            return Err(anyhow!(
                                "missing snapshot directory for nydus block volume"
                            ));
                        }
                    }
                }
            }
            KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_FS | KATA_VIRTUAL_VOLUME_LAYER_NYDUS_FS => {
                if self.source.is_empty() {
                    return Err(anyhow!("missing meta blob for nydus fs volume"));
                } else if self.fs_type.as_str() != "rafsv6" && self.fs_type.as_str() != "rafsv5" {
                    return Err(anyhow!("invalid filesystem for nydus fs volume"));
                }
                match self.nydus_image.as_ref() {
                    None => {
                        return Err(anyhow!(
                            "missing nydus configuration info for nydus block volume"
                        ))
                    }
                    Some(nydus) => {
                        if nydus.config.is_empty() {
                            return Err(anyhow!(
                                "missing configuration info for nydus block volume"
                            ));
                        } else if nydus.snapshot_dir.is_empty() {
                            return Err(anyhow!(
                                "missing snapshot directory for nydus block volume"
                            ));
                        }
                    }
                }
            }
            KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL => {
                if self.source.is_empty() {
                    return Err(anyhow!("missing image reference for guest pulling volume"));
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Serialize the virtual volume object to json.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserialize a virtual volume object from json string.
    pub fn from_json(value: &str) -> Result<Self> {
        let volume: KataVirtualVolume = serde_json::from_str(value)?;
        volume.validate()?;
        Ok(volume)
    }

    /// Serialize the virtual volume object to json and encode the string with base64.
    pub fn to_base64(&self) -> Result<String> {
        let json = self.to_json()?;
        Ok(base64::encode(json))
    }

    /// Decode and deserialize a virtual volume object from base64 encoded json string.
    pub fn from_base64(value: &str) -> Result<Self> {
        let json = base64::decode(value)?;
        let volume: KataVirtualVolume = serde_json::from_slice(&json)?;
        volume.validate()?;
        Ok(volume)
    }
}

impl TryFrom<&DirectVolumeMountInfo> for KataVirtualVolume {
    type Error = Error;

    fn try_from(value: &DirectVolumeMountInfo) -> std::result::Result<Self, Self::Error> {
        let volume_type = match value.volume_type.as_str() {
            "block" => KATA_VIRTUAL_VOLUME_DIRECT_BLOCK.to_string(),
            _ => {
                return Err(anyhow!(
                    "unknown directly assigned volume type: {}",
                    value.volume_type
                ))
            }
        };

        Ok(KataVirtualVolume {
            volume_type,
            source: value.device.clone(),
            fs_type: value.fs_type.clone(),
            options: value.options.clone(),
            direct_volume: Some(DirectAssignedVolume {
                metadata: value.metadata.clone(),
            }),
            ..Default::default()
        })
    }
}

impl TryFrom<&NydusExtraOptions> for KataVirtualVolume {
    type Error = Error;

    fn try_from(value: &NydusExtraOptions) -> std::result::Result<Self, Self::Error> {
        let fs_type = match value.fs_version.as_str() {
            "v6" => "rafsv6".to_string(),
            "rafsv6" => "rafsv6".to_string(),
            "v5" => "rafsv5".to_string(),
            "rafsv5" => "rafsv5".to_string(),
            _ => return Err(anyhow!("unknown RAFS version: {}", value.fs_version)),
        };

        Ok(KataVirtualVolume {
            volume_type: KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_FS.to_string(),
            source: value.source.clone(),
            fs_type,
            options: vec![],
            nydus_image: Some(NydusImageVolume {
                config: value.config.clone(),
                snapshot_dir: value.snapshot_dir.clone(),
            }),
            ..Default::default()
        })
    }
}

/// Trait object for storage device.
pub trait StorageDevice: Send + Sync {
    /// Path
    fn path(&self) -> Option<&str>;

    /// Clean up resources related to the storage device.
    fn cleanup(&self) -> Result<()>;
}

/// Join user provided volume path with kata direct-volume root path.
///
/// The `volume_path` is base64-url-encoded and then safely joined to the `prefix`
pub fn join_path(prefix: &str, volume_path: &str) -> Result<PathBuf> {
    if volume_path.is_empty() {
        return Err(anyhow!(std::io::ErrorKind::NotFound));
    }
    let b64_url_encoded_path = base64::encode_config(volume_path.as_bytes(), base64::URL_SAFE);

    Ok(safe_path::scoped_join(prefix, b64_url_encoded_path)?)
}

/// get DirectVolume mountInfo from mountinfo.json.
pub fn get_volume_mount_info(volume_path: &str) -> Result<DirectVolumeMountInfo> {
    let volume_path = join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path)?;
    let mount_info_file_path = volume_path.join(KATA_MOUNT_INFO_FILE_NAME);
    let mount_info_file = fs::read_to_string(mount_info_file_path)?;
    let mount_info: DirectVolumeMountInfo = serde_json::from_str(&mount_info_file)?;

    Ok(mount_info)
}

/// Check whether a mount type is a marker for Kata specific volume.
pub fn is_kata_special_volume(ty: &str) -> bool {
    ty.len() > KATA_VOLUME_TYPE_PREFIX.len() && ty.starts_with(KATA_VOLUME_TYPE_PREFIX)
}

/// Check whether a mount type is a marker for Kata guest mount volume.
pub fn is_kata_guest_mount_volume(ty: &str) -> bool {
    ty.len() > KATA_GUEST_MOUNT_PREFIX.len() && ty.starts_with(KATA_GUEST_MOUNT_PREFIX)
}

/// Check whether a mount type is a marker for Kata ephemeral volume.
pub fn is_kata_ephemeral_volume(ty: &str) -> bool {
    ty == KATA_EPHEMERAL_VOLUME_TYPE
}

/// Check whether a mount type is a marker for Kata hostdir volume.
pub fn is_kata_host_dir_volume(ty: &str) -> bool {
    ty == KATA_HOST_DIR_VOLUME_TYPE
}

/// sandbox bindmount format:  /path/to/dir, or /path/to/dir:ro[:rw]
/// the real path is without suffix ":ro" or ":rw".
pub fn split_bind_mounts(bindmount: &str) -> (&str, &str) {
    let (real_path, mode) = if bindmount.ends_with(SANDBOX_BIND_MOUNTS_RO) {
        (
            bindmount.trim_end_matches(SANDBOX_BIND_MOUNTS_RO),
            SANDBOX_BIND_MOUNTS_RO,
        )
    } else if bindmount.ends_with(SANDBOX_BIND_MOUNTS_RW) {
        (
            bindmount.trim_end_matches(SANDBOX_BIND_MOUNTS_RW),
            SANDBOX_BIND_MOUNTS_RW,
        )
    } else {
        // default bindmount format
        (bindmount, "")
    };

    (real_path, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_is_kata_special_volume() {
        assert!(is_kata_special_volume("kata:guest-mount:nfs"));
        assert!(!is_kata_special_volume("kata:"));
    }

    #[test]
    fn test_split_bind_mounts() {
        let test01 = "xxx0:ro";
        let test02 = "xxx2:rw";
        let test03 = "xxx3:is";
        let test04 = "xxx4";
        assert_eq!(split_bind_mounts(test01), ("xxx0", ":ro"));
        assert_eq!(split_bind_mounts(test02), ("xxx2", ":rw"));
        assert_eq!(split_bind_mounts(test03), ("xxx3:is", ""));
        assert_eq!(split_bind_mounts(test04), ("xxx4", ""));
    }

    #[test]
    fn test_is_kata_guest_mount_volume() {
        assert!(is_kata_guest_mount_volume("kata:guest-mount:nfs"));
        assert!(!is_kata_guest_mount_volume("kata:guest-mount"));
        assert!(!is_kata_guest_mount_volume("kata:guest-moun"));
        assert!(!is_kata_guest_mount_volume("Kata:guest-mount:nfs"));
    }

    #[test]
    fn test_get_nydus_extra_options_v5() {
        let mut mount_info = Mount {
            ..Default::default()
        };
        mount_info.options = vec!["extraoption=eyJzb3VyY2UiOiIvdmFyL2xpYi9jb250YWluZXJkL2lvLmNvbnRhaW5lcmQuc25hcHNob3R0ZXIudjEubnlkdXMvc25hcHNob3RzLzkvZnMvaW1hZ2UvaW1hZ2UuYm9vdCIsImNvbmZpZyI6IntcImRldmljZVwiOntcImJhY2tlbmRcIjp7XCJ0eXBlXCI6XCJyZWdpc3RyeVwiLFwiY29uZmlnXCI6e1wicmVhZGFoZWFkXCI6ZmFsc2UsXCJob3N0XCI6XCJsb2NhbGhvc3Q6NTAwMFwiLFwicmVwb1wiOlwidWJ1bnR1LW55ZHVzXCIsXCJzY2hlbWVcIjpcImh0dHBcIixcInNraXBfdmVyaWZ5XCI6dHJ1ZSxcInByb3h5XCI6e1wiZmFsbGJhY2tcIjpmYWxzZX0sXCJ0aW1lb3V0XCI6NSxcImNvbm5lY3RfdGltZW91dFwiOjUsXCJyZXRyeV9saW1pdFwiOjJ9fSxcImNhY2hlXCI6e1widHlwZVwiOlwiYmxvYmNhY2hlXCIsXCJjb25maWdcIjp7XCJ3b3JrX2RpclwiOlwiL3Zhci9saWIvbnlkdXMvY2FjaGVcIixcImRpc2FibGVfaW5kZXhlZF9tYXBcIjpmYWxzZX19fSxcIm1vZGVcIjpcImRpcmVjdFwiLFwiZGlnZXN0X3ZhbGlkYXRlXCI6ZmFsc2UsXCJlbmFibGVfeGF0dHJcIjp0cnVlLFwiZnNfcHJlZmV0Y2hcIjp7XCJlbmFibGVcIjp0cnVlLFwicHJlZmV0Y2hfYWxsXCI6ZmFsc2UsXCJ0aHJlYWRzX2NvdW50XCI6NCxcIm1lcmdpbmdfc2l6ZVwiOjAsXCJiYW5kd2lkdGhfcmF0ZVwiOjB9LFwidHlwZVwiOlwiXCIsXCJpZFwiOlwiXCIsXCJkb21haW5faWRcIjpcIlwiLFwiY29uZmlnXCI6e1wiaWRcIjpcIlwiLFwiYmFja2VuZF90eXBlXCI6XCJcIixcImJhY2tlbmRfY29uZmlnXCI6e1wicmVhZGFoZWFkXCI6ZmFsc2UsXCJwcm94eVwiOntcImZhbGxiYWNrXCI6ZmFsc2V9fSxcImNhY2hlX3R5cGVcIjpcIlwiLFwiY2FjaGVfY29uZmlnXCI6e1wid29ya19kaXJcIjpcIlwifSxcIm1ldGFkYXRhX3BhdGhcIjpcIlwifX0iLCJzbmFwc2hvdGRpciI6Ii92YXIvbGliL2NvbnRhaW5lcmQvaW8uY29udGFpbmVyZC5zbmFwc2hvdHRlci52MS5ueWR1cy9zbmFwc2hvdHMvMjU3IiwiZnNfdmVyc2lvbiI6InY1In0=".to_string()];
        let extra_option_result = NydusExtraOptions::new(&mount_info);
        assert!(extra_option_result.is_ok());
        let extra_option = extra_option_result.unwrap();
        assert_eq!(extra_option.source,"/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/9/fs/image/image.boot");
        assert_eq!(
            extra_option.snapshot_dir,
            "/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/257"
        );
        assert_eq!(extra_option.fs_version, "v5");
    }

    #[test]
    fn test_get_nydus_extra_options_v6() {
        let mut mount_info = Mount {
            ..Default::default()
        };
        mount_info.options = vec!["extraoption=eyJzb3VyY2UiOiIvdmFyL2xpYi9jb250YWluZXJkL2lvLmNvbnRhaW5lcmQuc25hcHNob3R0ZXIudjEubnlkdXMvc25hcHNob3RzLzIwMS9mcy9pbWFnZS9pbWFnZS5ib290IiwiY29uZmlnIjoie1wiZGV2aWNlXCI6e1wiYmFja2VuZFwiOntcInR5cGVcIjpcInJlZ2lzdHJ5XCIsXCJjb25maWdcIjp7XCJyZWFkYWhlYWRcIjpmYWxzZSxcImhvc3RcIjpcImxvY2FsaG9zdDo1MDAwXCIsXCJyZXBvXCI6XCJ1YnVudHUtbnlkdXMtdjZcIixcInNjaGVtZVwiOlwiaHR0cFwiLFwic2tpcF92ZXJpZnlcIjp0cnVlLFwicHJveHlcIjp7XCJmYWxsYmFja1wiOmZhbHNlfSxcInRpbWVvdXRcIjo1LFwiY29ubmVjdF90aW1lb3V0XCI6NSxcInJldHJ5X2xpbWl0XCI6Mn19LFwiY2FjaGVcIjp7XCJ0eXBlXCI6XCJibG9iY2FjaGVcIixcImNvbmZpZ1wiOntcIndvcmtfZGlyXCI6XCIvdmFyL2xpYi9ueWR1cy9jYWNoZVwiLFwiZGlzYWJsZV9pbmRleGVkX21hcFwiOmZhbHNlfX19LFwibW9kZVwiOlwiZGlyZWN0XCIsXCJkaWdlc3RfdmFsaWRhdGVcIjpmYWxzZSxcImVuYWJsZV94YXR0clwiOnRydWUsXCJmc19wcmVmZXRjaFwiOntcImVuYWJsZVwiOnRydWUsXCJwcmVmZXRjaF9hbGxcIjpmYWxzZSxcInRocmVhZHNfY291bnRcIjo0LFwibWVyZ2luZ19zaXplXCI6MCxcImJhbmR3aWR0aF9yYXRlXCI6MH0sXCJ0eXBlXCI6XCJcIixcImlkXCI6XCJcIixcImRvbWFpbl9pZFwiOlwiXCIsXCJjb25maWdcIjp7XCJpZFwiOlwiXCIsXCJiYWNrZW5kX3R5cGVcIjpcIlwiLFwiYmFja2VuZF9jb25maWdcIjp7XCJyZWFkYWhlYWRcIjpmYWxzZSxcInByb3h5XCI6e1wiZmFsbGJhY2tcIjpmYWxzZX19LFwiY2FjaGVfdHlwZVwiOlwiXCIsXCJjYWNoZV9jb25maWdcIjp7XCJ3b3JrX2RpclwiOlwiXCJ9LFwibWV0YWRhdGFfcGF0aFwiOlwiXCJ9fSIsInNuYXBzaG90ZGlyIjoiL3Zhci9saWIvY29udGFpbmVyZC9pby5jb250YWluZXJkLnNuYXBzaG90dGVyLnYxLm55ZHVzL3NuYXBzaG90cy8yNjEiLCJmc192ZXJzaW9uIjoidjYifQ==".to_string()];
        let extra_option_result = NydusExtraOptions::new(&mount_info);
        assert!(extra_option_result.is_ok());
        let extra_option = extra_option_result.unwrap();
        assert_eq!(extra_option.source,"/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/201/fs/image/image.boot");
        assert_eq!(
            extra_option.snapshot_dir,
            "/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/261"
        );
        assert_eq!(extra_option.fs_version, "v6");
    }

    #[test]
    fn test_kata_virtual_volume() {
        let mut volume = KataVirtualVolume::new(KATA_VIRTUAL_VOLUME_DIRECT_BLOCK.to_string());
        assert_eq!(
            volume.volume_type.as_str(),
            KATA_VIRTUAL_VOLUME_DIRECT_BLOCK
        );
        assert!(volume.fs_type.is_empty());

        let value = serde_json::to_string(&volume).unwrap();
        assert_eq!(&value, "{\"volume_type\":\"direct_block\"}");

        volume.source = "/tmp".to_string();
        volume.fs_type = "ext4".to_string();
        volume.options = vec!["rw".to_string()];
        volume.nydus_image = Some(NydusImageVolume {
            config: "test".to_string(),
            snapshot_dir: "/var/lib/nydus.dir".to_string(),
        });
        let mut metadata = HashMap::new();
        metadata.insert("mode".to_string(), "rw".to_string());
        volume.direct_volume = Some(DirectAssignedVolume { metadata });

        let value = serde_json::to_string(&volume).unwrap();
        let volume2: KataVirtualVolume = serde_json::from_str(&value).unwrap();
        assert_eq!(volume.volume_type, volume2.volume_type);
        assert_eq!(volume.source, volume2.source);
        assert_eq!(volume.fs_type, volume2.fs_type);
        assert_eq!(volume.nydus_image, volume2.nydus_image);
        assert_eq!(volume.direct_volume, volume2.direct_volume);
    }

    #[test]
    fn test_kata_virtual_volume_serde() {
        let mut volume = KataVirtualVolume::new(KATA_VIRTUAL_VOLUME_DIRECT_BLOCK.to_string());
        volume.source = "/tmp".to_string();
        volume.fs_type = "ext4".to_string();
        volume.options = vec!["rw".to_string()];
        volume.nydus_image = Some(NydusImageVolume {
            config: "test".to_string(),
            snapshot_dir: "/var/lib/nydus.dir".to_string(),
        });
        let mut metadata = HashMap::new();
        metadata.insert("mode".to_string(), "rw".to_string());
        volume.direct_volume = Some(DirectAssignedVolume { metadata });

        let value = volume.to_base64().unwrap();
        let volume2: KataVirtualVolume = KataVirtualVolume::from_base64(value.as_str()).unwrap();
        assert_eq!(volume.volume_type, volume2.volume_type);
        assert_eq!(volume.source, volume2.source);
        assert_eq!(volume.fs_type, volume2.fs_type);
        assert_eq!(volume.nydus_image, volume2.nydus_image);
        assert_eq!(volume.direct_volume, volume2.direct_volume);
    }

    #[test]
    fn test_try_from_direct_volume() {
        let mut metadata = HashMap::new();
        metadata.insert("mode".to_string(), "rw".to_string());
        let mut direct = DirectVolumeMountInfo {
            volume_type: "unknown".to_string(),
            device: "/dev/vda".to_string(),
            fs_type: "ext4".to_string(),
            metadata,
            options: vec!["ro".to_string()],
        };
        KataVirtualVolume::try_from(&direct).unwrap_err();

        direct.volume_type = "block".to_string();
        let volume = KataVirtualVolume::try_from(&direct).unwrap();
        assert_eq!(
            volume.volume_type.as_str(),
            KATA_VIRTUAL_VOLUME_DIRECT_BLOCK
        );
        assert_eq!(volume.source, direct.device);
        assert_eq!(volume.fs_type, direct.fs_type);
        assert_eq!(
            volume.direct_volume.as_ref().unwrap().metadata,
            direct.metadata
        );
        assert_eq!(volume.options, direct.options);
    }

    #[test]
    fn test_try_from_nydus_extra_options() {
        let mut nydus = NydusExtraOptions {
            source: "/test/nydus".to_string(),
            config: "test".to_string(),
            snapshot_dir: "/var/lib/nydus".to_string(),
            fs_version: "rafsvx".to_string(),
        };
        KataVirtualVolume::try_from(&nydus).unwrap_err();

        nydus.fs_version = "v6".to_string();
        let volume = KataVirtualVolume::try_from(&nydus).unwrap();
        assert_eq!(
            volume.volume_type.as_str(),
            KATA_VIRTUAL_VOLUME_IMAGE_NYDUS_FS
        );
        assert_eq!(volume.nydus_image.as_ref().unwrap().config, nydus.config);
        assert_eq!(
            volume.nydus_image.as_ref().unwrap().snapshot_dir,
            nydus.snapshot_dir
        );
        assert_eq!(volume.fs_type.as_str(), "rafsv6")
    }
}
