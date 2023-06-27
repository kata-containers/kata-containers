// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use std::{collections::HashMap, fs, path::PathBuf};

/// Prefix to mark a volume as Kata special.
pub const KATA_VOLUME_TYPE_PREFIX: &str = "kata:";

/// The Mount should be ignored by the host and handled by the guest.
pub const KATA_GUEST_MOUNT_PREFIX: &str = "kata:guest-mount:";

/// KATA_EPHEMERAL_DEV_TYPE creates a tmpfs backed volume for sharing files between containers.
pub const KATA_EPHEMERAL_VOLUME_TYPE: &str = "ephemeral";

/// KATA_HOST_DIR_TYPE use for host empty dir
pub const KATA_HOST_DIR_VOLUME_TYPE: &str = "kata:hostdir";

/// KATA_MOUNT_INFO_FILE_NAME is used for the file that holds direct-volume mount info
pub const KATA_MOUNT_INFO_FILE_NAME: &str = "mountInfo.json";

/// KATA_DIRECT_VOLUME_ROOT_PATH is the root path used for concatenating with the direct-volume mount info file path
pub const KATA_DIRECT_VOLUME_ROOT_PATH: &str = "/run/kata-containers/shared/direct-volumes";

/// SANDBOX_BIND_MOUNTS_DIR is for sandbox bindmounts
pub const SANDBOX_BIND_MOUNTS_DIR: &str = "sandbox-mounts";

/// SANDBOX_BIND_MOUNTS_RO is for sandbox bindmounts with readonly
pub const SANDBOX_BIND_MOUNTS_RO: &str = ":ro";

/// SANDBOX_BIND_MOUNTS_RO is for sandbox bindmounts with readwrite
pub const SANDBOX_BIND_MOUNTS_RW: &str = ":rw";

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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DirectVolumeMountInfo {
    /// The type of the volume (ie. block)
    pub volume_type: String,
    /// The device backing the volume.
    pub device: String,
    /// The filesystem type to be mounted on the volume.
    pub fs_type: String,
    /// Additional metadata to pass to the agent regarding this volume.
    pub metadata: HashMap<String, String>,
    /// Additional mount options.
    pub options: Vec<String>,
}

/// join_path joins user provided volumepath with kata direct-volume root path
/// the volume_path is base64-encoded and then safely joined to the end of path prefix
pub fn join_path(prefix: &str, volume_path: &str) -> Result<PathBuf> {
    if volume_path.is_empty() {
        return Err(anyhow!("volume path must not be empty"));
    }
    let b64_encoded_path = base64::encode(volume_path.as_bytes());

    Ok(safe_path::scoped_join(prefix, b64_encoded_path)?)
}

/// get DirectVolume mountInfo from mountinfo.json.
pub fn get_volume_mount_info(volume_path: &str) -> Result<DirectVolumeMountInfo> {
    let mount_info_file_path =
        join_path(KATA_DIRECT_VOLUME_ROOT_PATH, volume_path)?.join(KATA_MOUNT_INFO_FILE_NAME);
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
}
