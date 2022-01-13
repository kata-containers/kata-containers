// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/// Prefix to mark a volume as Kata special.
pub const KATA_VOLUME_TYPE_PREFIX: &str = "kata:";

/// The Mount should be ignored by the host and handled by the guest.
pub const KATA_GUEST_MOUNT_PREFIX: &str = "kata:guest-mount:";

/// KATA_EPHEMERAL_DEV_TYPE creates a tmpfs backed volume for sharing files between containers.
pub const KATA_EPHEMERAL_VOLUME_TYPE: &str = "kata:ephemeral";

/// KATA_HOST_DIR_TYPE use for host empty dir
pub const KATA_HOST_DIR_VOLUME_TYPE: &str = "kata:hostdir";

/// Information about a mount.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Mount {
    /// The source path for the mountpoint.
    pub source: String,
    /// The destination path for the mountpoint.
    pub destination: String,
    /// The type of filesystem for the mountpoint.
    pub fs_type: String,
    /// Mount options for the mountpoint.
    pub options: Vec<String>,
    /// Device id for device associated with the mountpoint.

    pub device_id: String,
    /// Host side bind mount path for the mountpoint.
    pub host_path: Option<String>,
    /// Whether to mount the mountpoint in readonly mode
    pub read_only: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_kata_special_volume() {
        assert!(is_kata_special_volume("kata:guest-mount:nfs"));
        assert!(!is_kata_special_volume("kata:"));
    }

    #[test]
    fn test_is_kata_guest_mount_volume() {
        assert!(is_kata_guest_mount_volume("kata:guest-mount:nfs"));
        assert!(!is_kata_guest_mount_volume("kata:guest-mount"));
        assert!(!is_kata_guest_mount_volume("kata:guest-moun"));
        assert!(!is_kata_guest_mount_volume("Kata:guest-mount:nfs"));
    }
}
